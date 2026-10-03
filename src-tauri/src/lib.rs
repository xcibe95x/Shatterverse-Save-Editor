#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::PathBuf, sync::Mutex};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

const MAGIC: &[u8; 4] = b"GVAS";
// A complete, self-contained `SeriousStoneData` property block (struct wrapper + nested
// array + 23 real trinkets) extracted byte-for-byte from a real, game-produced save.
// A brand-new save doesn't serialize this property at all when it owns zero trinkets, so
// there's nothing to clone from -- this lets a first trinket be added by splicing in this
// known-good blob instead, right after CodexEntries (verified to be exactly where this
// property sits in every save that has it).
const STARTER_TRINKETS: &[u8] = include_bytes!("../assets/starter_trinkets.bin");

#[derive(Clone, Serialize)]
struct Property {
    name: String,
    type_name: String,
    tag_at: usize,
    value_at: Option<usize>,
    value: Value,
    editable: bool,
    scope: String,
    note: String,
}

#[derive(Clone, Serialize)]
struct Currency {
    key: String,
    offset: usize,
    value: i32,
}

#[derive(Clone, Serialize)]
struct FlagGroup {
    key: String,
    count: usize,
    enabled: bool,
    mixed: bool,
    offsets: Vec<usize>,
}

#[derive(Clone, Serialize)]
struct Stone {
    index: usize,
    id: String,
    stone_set: String,
    level: u32,
    level_offset: usize,
    rank: u32,
    rank_offset: usize,
    foil: Option<u8>,
    foil_offset: Option<usize>,
    start: usize,
    end: usize,
    secondary: Vec<(String, i64)>,
}

#[derive(Clone, Serialize)]
struct Snapshot {
    path: String,
    filename: String,
    size: usize,
    currencies: Vec<Currency>,
    flags: Vec<FlagGroup>,
    stones: Vec<Stone>,
    properties: Vec<Property>,
    warnings: Vec<String>,
    changes: Vec<String>,
    has_unsaved_changes: bool,
    backup_path: Option<String>,
}

#[derive(Clone, Serialize)]
struct BackupInfo {
    filename: String,
    label: String,
    size: u64,
    modified_at: String,
}

/// Save-derived workshop entry. Instance ServerIds are deliberately excluded.
#[derive(Serialize, Deserialize)]
struct StoneObservation {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "Set")]
    stone_set: String,
    #[serde(rename = "Lvl")]
    level: u32,
    #[serde(rename = "R")]
    rank: u32,
    #[serde(rename = "Sh")]
    shape: i64,
    #[serde(rename = "B")]
    base_roll: i64,
    #[serde(rename = "M")]
    magnitude: i64,
    #[serde(rename = "SecS")]
    secondary: BTreeMap<String, i64>,
    #[serde(rename = "F")]
    foil: Option<u8>,
}

fn observation_from_block(block: &[u8]) -> Result<StoneObservation, String> {
    let field_i64 = |name: &str| -> Result<i64, String> {
        tagged_value_range(block, 0, block.len(), name, "IntProperty")
            .and_then(|(value, _)| value.as_i64())
            .ok_or_else(|| format!("Trinket export is missing a valid {name} value."))
    };
    let id = tagged_value_range(block, 0, block.len(), "ID", "NameProperty")
        .and_then(|(value, _)| value.as_str().map(str::to_owned))
        .ok_or("Trinket export is missing its ID.")?;
    let stone_set = tagged_value_range(block, 0, block.len(), "Set", "NameProperty")
        .and_then(|(value, _)| value.as_str().map(str::to_owned))
        .ok_or("Trinket export is missing its set name.")?;
    let level = tagged_value_range(block, 0, block.len(), "Lvl", "UInt32Property")
        .and_then(|(value, _)| value.as_u64()).ok_or("Trinket export is missing its level.")? as u32;
    let rank = tagged_value_range(block, 0, block.len(), "R", "UInt32Property")
        .and_then(|(value, _)| value.as_u64()).ok_or("Trinket export is missing its rank.")? as u32;
    let foil = tagged_value_range(block, 0, block.len(), "F", "ByteProperty")
        .and_then(|(value, _)| value.as_u64()).map(|value| value as u8);
    Ok(StoneObservation {
        id, stone_set, level, rank,
        shape: field_i64("Sh")?, base_roll: field_i64("B")?, magnitude: field_i64("M")?,
        secondary: read_secondary_entries(block, 0, block.len())?.into_iter().map(|(key, (value, _))| (key, value)).collect(), foil,
    })
}

#[derive(Deserialize)]
struct Edit {
    offset: usize,
    value: Value,
}

struct LoadedSave {
    path: PathBuf,
    original: Vec<u8>,
    data: Vec<u8>,
    currencies: Vec<Currency>,
    flags: Vec<FlagGroup>,
    stones: Vec<Stone>,
    properties: Vec<Property>,
    warnings: Vec<String>,
    changes: Vec<String>,
    backup_path: Option<PathBuf>,
    stone_count_at: Option<usize>,
    stone_array_size_at: Option<usize>,
    stone_struct_size_at: Option<usize>,
    stone_data_at: Option<usize>,
    stone_end: Option<usize>,
    currency_count_at: Option<usize>,
    currency_size_at: Option<usize>,
    currency_entries_end: Option<usize>,
    codex_array_end: Option<usize>,
}

impl LoadedSave {
    fn open(path: PathBuf) -> Result<Self, String> {
        let data = fs::read(&path).map_err(|e| format!("Could not read save: {e}"))?;
        if !data.starts_with(MAGIC) {
            return Err("This file does not have a GVAS save header.".into());
        }
        let mut loaded = Self {
            path,
            original: data.clone(),
            data,
            currencies: vec![],
            flags: vec![],
            stones: vec![],
            properties: vec![],
            warnings: vec![],
            changes: vec![],
            backup_path: None,
            stone_count_at: None,
            stone_array_size_at: None,
            stone_struct_size_at: None,
            stone_data_at: None,
            stone_end: None,
            currency_count_at: None,
            currency_size_at: None,
            currency_entries_end: None,
            codex_array_end: None,
        };
        loaded.scan();
        Ok(loaded)
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            path: self.path.display().to_string(),
            filename: self.path.file_name().and_then(|n| n.to_str()).unwrap_or("save.sav").to_string(),
            size: self.data.len(),
            currencies: self.currencies.clone(),
            flags: self.flags.clone(),
            stones: self.stones.clone(),
            properties: self.properties.clone(),
            warnings: self.warnings.clone(),
            changes: self.changes.clone(),
            has_unsaved_changes: self.data != self.original,
            backup_path: self.backup_path.as_ref().map(|p| p.display().to_string()),
        }
    }

    fn observations(&self) -> Result<Vec<StoneObservation>, String> {
        if self.stones.is_empty() { return Err("No trinket records were found in this save.".into()); }
        self.stones.iter().map(|stone| {
            let scope = format!("Stone {}", stone.index);
            let number = |name: &str| self.properties.iter()
                .find(|p| p.scope == scope && p.name == name)
                .and_then(|p| p.value.as_i64())
                .ok_or_else(|| format!("Trinket {} is missing a recognized {name} value.", stone.index));
            let secondary = stone.secondary.iter().cloned().collect::<BTreeMap<_, _>>();
            Ok(StoneObservation {
                id: stone.id.clone(), stone_set: stone.stone_set.clone(), level: stone.level, rank: stone.rank,
                shape: number("Sh")?, base_roll: number("B")?, magnitude: number("M")?, secondary, foil: stone.foil,
            })
        }).collect()
    }

    fn scan(&mut self) {
        self.scan_typed_properties();
        self.scan_currencies();
        self.scan_numeric_maps();
        self.scan_stones();
        self.scan_flags();
        self.scan_codex();
    }

    fn scan_typed_properties(&mut self) {
        let types = ["BoolProperty", "ByteProperty", "IntProperty", "UInt32Property", "Int64Property", "FloatProperty", "NameProperty", "StrProperty", "ArrayProperty", "MapProperty", "StructProperty"];
        let mut seen = Vec::new();
        for type_name in types {
            let pattern = ftag(type_name);
            for type_at in find_all(&self.data, &pattern) {
                let lower = type_at.saturating_sub(164);
                for start in lower..type_at {
                    if seen.contains(&start) { continue; }
                    if let Some((name, after_name)) = read_fstr(&self.data, start) {
                        if name.is_empty() || after_name != type_at || !name.chars().all(|c| !c.is_control()) { continue; }
                        if let Some(prop) = parse_property(&self.data, start) {
                            seen.push(start);
                            self.properties.push(prop);
                        }
                    }
                }
            }
        }
    }

    fn scan_currencies(&mut self) {
        let map_tag = [ftag("Currencies"), ftag("MapProperty")].concat();
        let Some(map_at) = find_bytes(&self.data, &map_tag) else {
            self.warnings.push("Currencies map tag was not found.".into());
            return;
        };
        let map_end = map_at + map_tag.len();
        let key_ty = ftag("StrProperty");
        let val_ty = ftag("IntProperty");
        let Some(key_at) = find_bytes_from(&self.data, &key_ty, map_end) else {
            self.warnings.push("Currency map key type was not recognized.".into());
            return;
        };
        let Some(val_at) = find_bytes_from(&self.data, &val_ty, key_at + key_ty.len()) else {
            self.warnings.push("Currency map value type was not recognized.".into());
            return;
        };
        // The map can have 1-3 entries (a brand-new save only serializes currencies
        // that have actually been touched -- e.g. count=1 with just "CT" present when
        // GD/SD have never been earned yet), and whichever keys ARE present can be in
        // any order (UE's TMap does not guarantee insertion order). So: scan for a
        // plausible small count followed by a recognized key, then read exactly that
        // many entries.
        let search_from = val_at + val_ty.len();
        let scan_end = (search_from + 48).min(self.data.len().saturating_sub(4));
        let mut found_header = None;
        for candidate in search_from..scan_end {
            let Some(count) = get_u32(&self.data, candidate) else { continue; };
            if count == 0 || count > 3 { continue; }
            if let Some((key, _)) = read_fstr(&self.data, candidate + 4) {
                if ["CT", "GD", "SD"].contains(&key.as_str()) { found_header = Some((candidate, count)); break; }
            }
        }
        let Some((count_at, count)) = found_header else {
            self.warnings.push("Currency entries could not be validated against the map count.".into());
            return;
        };
        let mut pos = count_at + 4;
        let mut found = Vec::new();
        for _ in 0..count {
            let Some((key, next)) = read_fstr(&self.data, pos) else { found.clear(); break; };
            if next + 4 > self.data.len() { found.clear(); break; }
            if ["CT", "GD", "SD"].contains(&key.as_str()) {
                if let Some(value) = get_i32(&self.data, next) {
                    found.push(Currency { key: key.clone(), offset: next, value });
                    self.properties.push(Property { name: key, type_name: "Map<IntProperty>".into(), tag_at: pos, value_at: Some(next), value: json!(value), editable: value >= 0, scope: "Currencies".into(), note: String::new() });
                }
            }
            pos = next + 4;
        }
        if found.len() as u32 == count && !found.is_empty() {
            found.sort_by_key(|c| ["CT", "SD", "GD"].iter().position(|k| *k == c.key).unwrap_or(9));
            self.currencies = found;
            // Size sits 8 bytes before the count field (ArrayIndex[4] + Size[4] + Reserved[4] + Count[4]),
            // verified against two real saves with 1 and 3 entries (declared size == 4 + 4 + sum(entry bytes)).
            self.currency_count_at = Some(count_at);
            // Verified byte-exact against a real save: ArrayIndex(4) + Size(4) + 5 zero
            // padding bytes + Count(4) -- Size sits 9 bytes before Count, not 8.
            self.currency_size_at = count_at.checked_sub(9);
            self.currency_entries_end = Some(pos);
            if count < 3 {
                let missing: Vec<&str> = ["CT", "GD", "SD"].iter().filter(|k| !self.currencies.iter().any(|c| &c.key == *k)).copied().collect();
                self.warnings.push(format!("This save has not earned {} yet, so it isn't in the file.", missing.join(" or ")));
            }
        } else {
            self.currencies.clear();
            self.warnings.push("Currency map entries did not match CT, GD, or SD.".into());
        }
    }

    fn scan_numeric_maps(&mut self) {
        let map_props: Vec<(usize, String)> = self.properties.iter().filter(|p| p.type_name == "MapProperty").map(|p| (p.tag_at, p.name.clone())).collect();
        for (tag_at, map_name) in map_props {
            let Some((_, name_end)) = read_fstr(&self.data, tag_at) else { continue; };
            let Some((_, type_end)) = read_fstr(&self.data, name_end) else { continue; };
            let mut pos = type_end + 4;
            let Some((key_ty, next)) = read_fstr(&self.data, pos) else { continue; };
            pos = next + 4;
            let Some((value_ty, next)) = read_fstr(&self.data, pos) else { continue; };
            pos = next;
            let value_width = match value_ty.as_str() { "IntProperty" | "UInt32Property" | "FloatProperty" => 4, "Int64Property" => 8, _ => continue };
            if key_ty != "NameProperty" && key_ty != "StrProperty" { continue; }
            let mut decoded = None;
            for count_at in pos..(pos + 40).min(self.data.len().saturating_sub(4)) {
                let Some(count) = get_u32(&self.data, count_at) else { continue; };
                if count > 500 { continue; }
                let mut cursor = count_at + 4;
                let mut entries = Vec::new();
                let mut valid = true;
                for _ in 0..count {
                    let Some((key, after_key)) = read_fstr(&self.data, cursor) else { valid = false; break; };
                    if after_key + value_width > self.data.len() { valid = false; break; }
                    let value_at = after_key;
                    let value = match value_ty.as_str() {
                        "IntProperty" => get_i32(&self.data, value_at).map(|v| json!(v)),
                        "UInt32Property" => get_u32(&self.data, value_at).map(|v| json!(v)),
                        "Int64Property" => get_i64(&self.data, value_at).map(|v| json!(v)),
                        "FloatProperty" => get_f32(&self.data, value_at).map(|v| json!(v)),
                        _ => None,
                    };
                    let Some(value) = value else { valid = false; break; };
                    entries.push(Property { name: format!("{map_name}[{key}]"), type_name: format!("Map<{value_ty}>"), tag_at, value_at: Some(value_at), value, editable: value_ty != "FloatProperty", scope: String::new(), note: format!("key={key}") });
                    cursor = value_at + value_width;
                }
                if valid && (parse_property(&self.data, cursor).is_some() || self.data.get(cursor..cursor + ftag("None").len()) == Some(ftag("None").as_slice())) {
                    decoded = Some(entries);
                    break;
                }
            }
            if let Some(entries) = decoded { self.properties.extend(entries); }
        }
    }

    fn scan_flags(&mut self) {
        for key in ["bCC", "bCl", "bL"] {
            let mut offsets = Vec::new();
            let pat = [ftag(key), ftag("BoolProperty")].concat();
            for at in find_all(&self.data, &pat) {
                if let Some(prop) = parse_property(&self.data, at) {
                    if prop.type_name == "BoolProperty" && prop.value_at.is_some() && (prop.value == json!(0) || prop.value == json!(16)) { offsets.push(prop.value_at.unwrap()); }
                }
            }
            if offsets.is_empty() { continue; }
            let values: Vec<u8> = offsets.iter().filter_map(|&o| self.data.get(o).copied()).collect();
            let all_true = values.iter().all(|v| *v != 0);
            let all_false = values.iter().all(|v| *v == 0);
            self.flags.push(FlagGroup { key: key.into(), count: offsets.len(), enabled: all_true, mixed: !(all_true || all_false), offsets });
        }
    }

    /// CodexEntries is one array shared by three gameplay systems: Boons (stacking
    /// power-ups), simple one-off unlocks (characters, difficulty tiers, weapons...),
    /// and the real Codex (content entries with CP/bCC/bCl challenge tracking). They
    /// all use the same `BowSerializableCodexEntry` struct, distinguished only by
    /// whether a nested `CQP` challenge array is present. This reuses the already
    /// globally-scanned properties (like scan_stones does) and just labels their
    /// `scope` so the UI can group "Unlock: <id>" vs "Codex: <id>" sensibly.
    fn scan_codex(&mut self) {
        let anchor = ftag("CodexEntries");
        let Some(aidx) = find_bytes(&self.data, &anchor) else { return; };
        let mut pos = aidx + anchor.len();
        let Some((type_str, next)) = read_fstr(&self.data, pos) else { return; };
        if type_str != "ArrayProperty" { return; }
        pos = next + 4;
        let Some((inner_type, next)) = read_fstr(&self.data, pos) else { return; };
        if inner_type != "StructProperty" { return; }
        pos = next + 4;
        let Some((_, next)) = read_fstr(&self.data, pos) else { return; }; // struct name
        pos = next + 4;
        let Some((_, next)) = read_fstr(&self.data, pos) else { return; }; // path
        pos = next;
        // Layout after the path string: 4-byte zero/reserved, 4-byte declared size,
        // 1-byte zero, 4-byte count (verified against real saves -- declared size is
        // NOT at `pos`, it's 4 bytes further in, after the reserved dword).
        let Some(declared_size) = get_u32(&self.data, pos + 4) else { return; };
        let Some(count) = get_u32(&self.data, pos + 9) else { return; };
        let data_at = pos + 13;
        let array_end = data_at + declared_size as usize;
        if array_end > self.data.len() { self.warnings.push("CodexEntries array size is outside the save; unlocks are read-only.".into()); return; }
        self.codex_array_end = Some(array_end);
        let id_tag = [ftag("ID"), ftag("NameProperty")].concat();
        let starts: Vec<usize> = find_all(&self.data, &id_tag).into_iter().filter(|at| *at >= data_at && *at < array_end).collect();
        let count = count as usize;
        if starts.len() != count { self.warnings.push(format!("CodexEntries declared count is {count}, found {} record anchors; unlocks are read-only.", starts.len())); return; }
        let cqp_tag = [ftag("CQP"), ftag("ArrayProperty")].concat();
        for i in 0..count {
            let start = starts[i];
            let next_start = if i + 1 < count { starts[i + 1] } else { array_end };
            let Some((id_val, _)) = tagged_value_range(&self.data, start, next_start, "ID", "NameProperty") else { continue; };
            let id = id_val.as_str().unwrap_or_default().to_string();
            // The CQP field is always PRESENT (even on simple unlocks like Sams and
            // weapons), just with zero elements -- checking tag presence alone
            // misclassified every entry as "real Codex". Must check the array's own
            // element count instead.
            let has_challenges = find_bytes_range(&self.data, &cqp_tag, start, next_start)
                .and_then(|cqp_at| {
                    let mut p = cqp_at + cqp_tag.len() + 4; // skip "1" marker after ArrayProperty type
                    let (inner_type, next) = read_fstr(&self.data, p)?; p = next + 4;
                    if inner_type != "StructProperty" { return None; }
                    let (_, next) = read_fstr(&self.data, p)?; p = next + 4; // struct name
                    let (_, next) = read_fstr(&self.data, p)?; // path
                    get_u32(&self.data, next + 9)
                })
                .is_some_and(|count| count > 0);
            let label = if has_challenges { format!("Codex: {id}") } else { format!("Unlock: {id}") };
            for property in &mut self.properties {
                if property.tag_at >= start && property.tag_at < next_start { property.scope = label.clone(); }
            }
        }
    }

    fn scan_stones(&mut self) {
        let marker = ftag("BowSerializableSeriousStoneData");
        let Some(marker_at) = find_bytes(&self.data, &marker) else { self.warnings.push("SeriousStoneData array count was not recognized; stone edits disabled.".into()); return; };
        let Some((_, mut pos)) = read_fstr(&self.data, marker_at) else { return; };
        pos += 4;
        let Some((_, pos)) = read_fstr(&self.data, pos) else { return; };
        let id_tag = [ftag("ID"), ftag("NameProperty")].concat();
        let mut count_at = None;
        for candidate in pos..(pos + 32).min(self.data.len().saturating_sub(4)) {
            if let Some(count) = get_u32(&self.data, candidate) {
                if count > 0 && count <= 500 && self.data.get(candidate + 4..candidate + 4 + id_tag.len()) == Some(id_tag.as_slice()) { count_at = Some(candidate); break; }
            }
        }
        let Some(count_at) = count_at else { self.warnings.push("Stone count header was not recognized.".into()); return; };
        let data_at = count_at + 4;
        let size_at = count_at.saturating_sub(5);
        let Some(array_size) = get_u32(&self.data, size_at).map(|n| n as usize) else { return; };
        let array_end = data_at.saturating_add(array_size);
        if array_end > self.data.len() || array_size == 0 { self.warnings.push("Stone array size is outside the save; stone edits disabled.".into()); return; }
        let array_tag = [ftag("SeriousStoneData"), ftag("ArrayProperty")].concat();
        let Some(array_at) = rfind_bytes_before(&self.data, &array_tag, count_at) else { self.warnings.push("Named stone array tag not found.".into()); return; };
        let struct_tag = [ftag("SeriousStoneData"), ftag("StructProperty")].concat();
        let Some(struct_at) = rfind_bytes_before(&self.data, &struct_tag, array_at) else { self.warnings.push("Parent stone struct not found; array changes disabled.".into()); return; };
        let Some((_, a)) = read_fstr(&self.data, struct_at) else { return; };
        let Some((_, type_end)) = read_fstr(&self.data, a) else { return; };
        let Some((_, class_end)) = read_fstr(&self.data, type_end + 4) else { return; };
        let Some((_, script_end)) = read_fstr(&self.data, class_end + 4) else { return; };
        let struct_size_at = script_end + 4;
        if struct_size_at + 4 > self.data.len() { return; }
        let starts = find_all(&self.data, &id_tag).into_iter().filter(|at| *at >= data_at && *at < array_end).collect::<Vec<_>>();
        let count = get_u32(&self.data, count_at).unwrap_or(0) as usize;
        if starts.len() != count { self.warnings.push(format!("Declared stone count is {count}, found {} record anchors; stone edits disabled.", starts.len())); return; }
        let mut stones = Vec::new();
        for index in 0..count {
            let start = starts[index];
            let next_start = if index + 1 < count { starts[index + 1] } else { array_end };
            for property in &mut self.properties {
                if property.tag_at >= start && property.tag_at < next_start { property.scope = format!("Stone {}", index + 1); }
            }
            let id = tagged_value_range(&self.data, start, next_start, "ID", "NameProperty");
            let server_set = tagged_value_range(&self.data, start, next_start, "Set", "NameProperty");
            let level = tagged_value_range(&self.data, start, next_start, "Lvl", "UInt32Property");
            let rank = tagged_value_range(&self.data, start, next_start, "R", "UInt32Property");
            let foil = tagged_value_range(&self.data, start, next_start, "F", "ByteProperty");
            let (Some((id, _)), Some((stone_set, _)), Some((level, level_offset)), Some((rank, rank_offset))) = (id, server_set, level, rank) else {
                self.warnings.push(format!("Stone {} fields were not recognized; stone edits disabled.", index + 1));
                self.stones.clear(); return;
            };
            let (end, foil_value, foil_offset) = if index + 1 < count {
                (next_start, foil.as_ref().and_then(|(v, _)| v.as_u64().map(|n| n as u8)), foil.as_ref().map(|(_, o)| *o))
            } else if let Some((fval, foff)) = foil {
                let none = ftag("None");
                let after_foil = foff + 1;
                let end = find_bytes_range(&self.data, &none, after_foil, array_end).map(|n| n + none.len()).unwrap_or(array_end);
                (end, fval.as_u64().map(|n| n as u8), Some(foff))
            } else { (next_start, None, None) };
            let secondary = self.properties.iter().filter(|p| p.scope == format!("Stone {}", index + 1) && p.name.starts_with("SecS[")).filter_map(|p| Some((p.note.strip_prefix("key=")?.to_string(), p.value.as_i64()?))).collect();
            let id = id.as_str().unwrap_or_default().to_string();
            let stone_set = stone_set.as_str().unwrap_or_default().to_string();
            stones.push(Stone { index: index + 1, id, stone_set, level: level.as_u64().unwrap_or(0) as u32, level_offset, rank: rank.as_u64().unwrap_or(0) as u32, rank_offset, foil: foil_value, foil_offset, start, end, secondary });
        }
        self.stones = stones;
        self.stone_count_at = Some(count_at);
        self.stone_array_size_at = Some(size_at);
        self.stone_struct_size_at = Some(struct_size_at);
        self.stone_data_at = Some(data_at);
        self.stone_end = Some(array_end);
    }

    fn stage(&mut self, edits: Vec<Edit>) -> Result<Snapshot, String> {
        let mut unique = Vec::new();
        for edit in edits {
            if !unique.iter().any(|e: &Edit| e.offset == edit.offset) { unique.push(edit); }
        }
        // Keep the transaction atomic, but only refresh the fields that were actually edited.
        // The former full-property walk decoded every scalar after every keystroke.
        let mut updated = self.data.clone();
        let mut changes = Vec::new();
        let mut touched = Vec::new();
        for edit in unique {
            let property_index = self.properties.iter().position(|p| p.value_at == Some(edit.offset) && p.editable)
                .ok_or_else(|| format!("Offset 0x{:08X} is not a recognized editable field.", edit.offset))?;
            let prop = self.properties[property_index].clone();
            let old = prop.value.clone();
            write_property(&mut updated, &prop, &edit.value)?;
            let changed = read_property_value(&updated, &prop)?;
            if old != changed {
                let prefix = if prop.scope.is_empty() { String::new() } else { format!("{} / ", prop.scope) };
                changes.push(format!("{}{} @0x{:08X}: {} → {}", prefix, prop.name, edit.offset, old, changed));
            }
            touched.push((property_index, edit.offset, changed));
        }
        self.data = updated;
        let touched_offsets = touched.iter().map(|(_, offset, _)| *offset).collect::<Vec<_>>();
        for (property_index, _, value) in &touched {
            self.properties[*property_index].value = value.clone();
        }
        for currency in &mut self.currencies {
            if touched_offsets.contains(&currency.offset) {
                if let Some(value) = get_i32(&self.data, currency.offset) { currency.value = value; }
            }
        }
        for flag in &mut self.flags {
            if !flag.offsets.iter().any(|offset| touched_offsets.contains(offset)) { continue; }
            let states: Vec<bool> = flag.offsets.iter().filter_map(|offset| self.data.get(*offset).map(|v| *v != 0)).collect();
            flag.enabled = !states.is_empty() && states.iter().all(|v| *v);
            flag.mixed = states.iter().any(|v| *v) && states.iter().any(|v| !*v);
        }
        for (property_index, _, value) in &touched {
            let prop = &self.properties[*property_index];
            if !prop.scope.starts_with("Stone ") { continue; }
            let Ok(index) = prop.scope[6..].parse::<usize>() else { continue; };
            let Some(stone) = self.stones.iter_mut().find(|stone| stone.index == index) else { continue; };
            match prop.name.as_str() {
                "Lvl" => if let Some(value) = value.as_u64() { stone.level = value as u32; },
                "R" => if let Some(value) = value.as_u64() { stone.rank = value as u32; },
                "F" => stone.foil = value.as_u64().map(|v| v as u8),
                name if name.starts_with("SecS[") && name.ends_with(']') => {
                    let key = &name[5..name.len()-1];
                    if let Some((_, current)) = stone.secondary.iter_mut().find(|(name, _)| name == key) {
                        if let Some(value) = value.as_i64() { *current = value; }
                    }
                }
                _ => {}
            }
        }
        self.changes.extend(changes);
        Ok(self.snapshot())
    }

    fn rescan_preserving_changes(&mut self) {
        let changes = self.changes.clone();
        self.currencies.clear(); self.flags.clear(); self.stones.clear(); self.properties.clear(); self.warnings.clear();
        self.stone_count_at = None; self.stone_array_size_at = None; self.stone_struct_size_at = None; self.stone_data_at = None; self.stone_end = None;
        self.currency_count_at = None; self.currency_size_at = None; self.currency_entries_end = None;
        self.codex_array_end = None;
        self.scan();
        self.changes = changes;
    }

    fn seed_trinkets(&mut self) -> Result<Snapshot, String> {
        if !self.stones.is_empty() { return Err("This save already has trinkets.".into()); }
        let Some(array_end) = self.codex_array_end else { return Err("Codex data was not validated; adding a starter set is disabled.".into()); };
        let insert_at = array_end + 5; // skip the "None" that closes the enclosing struct
        if insert_at > self.data.len() { return Err("Insertion point is outside the save.".into()); }
        let mut blob = STARTER_TRINKETS.to_vec();
        // Give each imported trinket a fresh ServerId instead of reusing the donor save's.
        let server_tag = [ftag("ServerId"), ftag("StrProperty")].concat();
        let mut search_pos = 0;
        while let Some(at) = find_bytes_from(&blob, &server_tag, search_pos) {
            let payload = at + server_tag.len() + 8;
            let Some(&guid_flag) = blob.get(payload) else { break; };
            let value_at = payload + 1 + if guid_flag == 1 { 16 } else { 0 };
            let Some(len) = get_i32(&blob, value_at) else { break; };
            if len == 33 {
                let new_guid = format!("{:032X}", rand_guid());
                blob[value_at + 4..value_at + 4 + 32].copy_from_slice(new_guid.as_bytes());
            }
            search_pos = value_at + 4 + len.max(0) as usize;
        }
        self.data.splice(insert_at..insert_at, blob);
        self.changes.push("Imported a starter set of 23 real trinkets (from community save data) since this save had none.".into());
        self.rescan_preserving_changes();
        Ok(self.snapshot())
    }

    fn add_stone(&mut self, template_index: usize, observation: StoneObservation) -> Result<Snapshot, String> {
        let (Some(count_at), Some(size_at), Some(struct_size_at), Some(array_end)) = (self.stone_count_at, self.stone_array_size_at, self.stone_struct_size_at, self.stone_end) else { return Err("Stone array size metadata was not validated; adding is disabled.".into()); };
        let StoneObservation { id, stone_set, level, rank, shape, base_roll, magnitude, secondary, foil } = observation;
        if id.is_empty() || stone_set.is_empty() || id.len() > 128 || stone_set.len() > 128 || id.contains('\0') || stone_set.contains('\0') { return Err("Use short, plain-text stone IDs and sets.".into()); }
        if foil.is_some_and(|v| v > 1) { return Err("Unknown flag must be 0 or 1.".into()); }
        let template = self.stones.iter().find(|s| s.index == template_index).cloned().ok_or("Choose an existing stone as the clone template.")?;
        let mut block = self.data[template.start..template.end].to_vec();
        patch_fstring_field(&mut block, "ID", "NameProperty", &id)?;
        patch_fstring_field(&mut block, "Set", "NameProperty", &stone_set)?;
        patch_fstring_field(&mut block, "ServerId", "StrProperty", &format!("{:032X}", rand_guid()))?;
        patch_u32_field(&mut block, "Lvl", "UInt32Property", level)?;
        patch_u32_field(&mut block, "R", "UInt32Property", rank)?;
        patch_i32_field(&mut block, "Sh", "IntProperty", i32::try_from(shape).map_err(|_| "Shape value is outside Int32 range.")?)?;
        patch_i32_field(&mut block, "B", "IntProperty", i32::try_from(base_roll).map_err(|_| "Base roll is outside Int32 range.")?)?;
        patch_i32_field(&mut block, "M", "IntProperty", i32::try_from(magnitude).map_err(|_| "Magnitude is outside Int32 range.")?)?;
        patch_secondary_map(&mut block, &secondary)?;
        if let Some(v) = foil { patch_byte_field(&mut block, "F", "ByteProperty", v)?; }
        let amount = block.len();
        let mut updated = self.data.clone();
        updated.splice(array_end..array_end, block);
        put_u32(&mut updated, count_at, self.stones.len() as u32 + 1)?;
        let old_array = get_u32(&updated, size_at).unwrap_or(0);
        let old_struct = get_u32(&updated, struct_size_at).unwrap_or(0);
        put_u32(&mut updated, size_at, old_array.checked_add(amount as u32).ok_or("Stone array size overflow.")?)?;
        put_u32(&mut updated, struct_size_at, old_struct.checked_add(amount as u32).ok_or("Stone struct size overflow.")?)?;
        self.data = updated;
        self.changes.push(format!("Added gathered trinket: {id} ({stone_set}), template #{}", template_index));
        self.rescan_preserving_changes();
        Ok(self.snapshot())
    }

    /// Returns the raw bytes of one owned trinket exactly as stored in the save --
    /// the same byte range `add_stone` would copy from when cloning. A `.trinket`
    /// file is just this blob; importing it is the same splice-and-patch-size
    /// operation as cloning, just sourced from a file instead of another record.
    fn export_trinket(&self, index: usize) -> Result<Vec<u8>, String> {
        let stone = self.stones.iter().find(|s| s.index == index).ok_or("Selected trinket was not found.")?;
        Ok(self.data[stone.start..stone.end].to_vec())
    }

    fn import_trinket(&mut self, bytes: Vec<u8>) -> Result<Snapshot, String> {
        let (Some(count_at), Some(size_at), Some(struct_size_at), Some(array_end)) = (self.stone_count_at, self.stone_array_size_at, self.stone_struct_size_at, self.stone_end) else { return Err("This save has no trinkets to import into yet -- use \"Import starter trinkets\" first.".into()); };
        let id_tag = [ftag("ID"), ftag("NameProperty")].concat();
        if !bytes.starts_with(&id_tag) { return Err("That file doesn't look like a valid trinket export (missing ID field at the start).".into()); }
        let server_tag = [ftag("ServerId"), ftag("StrProperty")].concat();
        if find_bytes(&bytes, &server_tag).is_none() { return Err("That file doesn't look like a valid trinket export (missing ServerId field).".into()); }
        let mut block = bytes;
        patch_fstring_field(&mut block, "ServerId", "StrProperty", &format!("{:032X}", rand_guid()))?;
        let amount = block.len();
        self.data.splice(array_end..array_end, block);
        put_u32(&mut self.data, count_at, self.stones.len() as u32 + 1)?;
        let old_array = get_u32(&self.data, size_at).unwrap_or(0);
        let old_struct = get_u32(&self.data, struct_size_at).unwrap_or(0);
        put_u32(&mut self.data, size_at, old_array.checked_add(amount as u32).ok_or("Stone array size overflow.")?)?;
        put_u32(&mut self.data, struct_size_at, old_struct.checked_add(amount as u32).ok_or("Stone struct size overflow.")?)?;
        self.changes.push("Imported a trinket from file".into());
        self.rescan_preserving_changes();
        Ok(self.snapshot())
    }

    fn replace_stone(&mut self, index: usize, template: Value) -> Result<Snapshot, String> {
        let stone = self.stones.iter().find(|s| s.index == index).cloned().ok_or("Choose an existing trinket slot.")?;
        let id = template.get("ID").and_then(Value::as_str).ok_or("Template ID is missing.")?;
        let set = template.get("Set").and_then(Value::as_str).ok_or("Template set is missing.")?;
        if id.contains('\0') || set.contains('\0') { return Err("Template text contains an unsupported null character.".into()); }
        let sec = template.get("SecS").and_then(Value::as_object).ok_or("Template secondary-stat layout is missing.")?;
        let mut existing_keys = stone.secondary.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
        let mut template_keys = sec.keys().cloned().collect::<Vec<_>>();
        existing_keys.sort(); template_keys.sort();
        if existing_keys != template_keys { return Err("This template has a different bonus-stat layout. Pick a template matching this slot's existing bonus-stat keys.".into()); }
        if id.as_bytes().len() != stone.id.as_bytes().len() || set.as_bytes().len() != stone.stone_set.as_bytes().len() {
            return Err("This template's ID or set needs a different string length. Choose a compatible slot; save strings cannot be resized safely.".into());
        }
        let scope = format!("Stone {index}");
        let mut updated = self.data.clone();
        let write_string = |updated: &mut [u8], field: &str, value: &str| -> Result<(), String> {
            let p = self.properties.iter().find(|p| p.scope == scope && p.name == field && p.type_name == "NameProperty").ok_or_else(|| format!("Slot {index} is missing its {field} property."))?;
            let at = p.value_at.ok_or_else(|| format!("Slot {index} {field} property has no string offset."))?;
            let (old, end) = read_fstr(updated, at).ok_or_else(|| format!("Slot {index} {field} string is not a validated UTF-8 FString."))?;
            if old.len() != value.len() || end != at + 4 + old.len() + 1 || updated.get(end - 1) != Some(&0) {
                return Err(format!("Slot {index} {field} string encoding or length is not compatible."));
            }
            updated.get_mut(at + 4..at + 4 + value.len()).ok_or("String payload is outside the save.")?.copy_from_slice(value.as_bytes());
            Ok(())
        };
        write_string(&mut updated, "ID", id)?;
        write_string(&mut updated, "Set", set)?;
        let numeric = [
            ("Lvl", "UInt32Property", template.get("Lvl").cloned().ok_or("Template level is invalid.")?),
            ("R", "UInt32Property", template.get("R").cloned().ok_or("Template rank is invalid.")?),
            ("Sh", "IntProperty", template.get("Sh").cloned().ok_or("Template shape is invalid.")?),
            ("B", "IntProperty", template.get("B").cloned().ok_or("Template base value is invalid.")?),
            ("M", "IntProperty", template.get("M").cloned().ok_or("Template magnitude is invalid.")?),
        ];
        for (name, ty, value) in numeric {
            let p = self.properties.iter().find(|p| p.scope == scope && p.name == name && p.type_name == ty).ok_or_else(|| format!("Slot {index} is missing compatible {name} data."))?;
            write_property(&mut updated, p, &value)?;
        }
        if let Some(foil) = template.get("F").and_then(Value::as_u64) {
            if foil > 1 { return Err("Template F value must be 0 or 1.".into()); }
            let p = self.properties.iter().find(|p| p.scope == scope && p.name == "F" && p.type_name == "ByteProperty").ok_or("This slot has no compatible F field.")?;
            write_property(&mut updated, p, &json!(foil))?;
        }
        for (key, value) in sec {
            let value = value.as_i64().ok_or_else(|| format!("Template secondary stat {key} is not an integer."))?;
            let name = format!("SecS[{key}]");
            let p = self.properties.iter().find(|p| p.scope == scope && p.name == name && p.type_name == "IntProperty").ok_or_else(|| format!("Slot {index} is missing secondary stat {key}."))?;
            write_property(&mut updated, p, &json!(value))?;
        }
        if updated.len() != self.data.len() { return Err("Internal safety check failed: replacement changed save length.".into()); }
        self.data = updated;
        self.changes.push(format!("Replaced trinket slot #{index}: {} → {} ({})", stone.id, id, set));
        self.rescan_preserving_changes();
        Ok(self.snapshot())
    }

    /// Rarity is encoded as the single trailing letter of a trinket's `ID` (e.g.
    /// `WeaponAoEDamage_E` = Epic). Swapping it is a same-size, in-place byte
    /// overwrite -- the ID string's length never changes -- so unlike cloning or
    /// adding, this carries no resize risk at all.
    fn set_trinket_rarity(&mut self, index: usize, letter: char) -> Result<Snapshot, String> {
        if !"CUREL".contains(letter) { return Err("Rarity letter must be one of C, U, R, E, L.".into()); }
        let stone = self.stones.iter().find(|s| s.index == index).cloned().ok_or("Selected trinket was not found.")?;
        let (id_value, id_value_at) = tagged_value_range(&self.data, stone.start, stone.end, "ID", "NameProperty").ok_or("Could not locate this trinket's ID field.")?;
        let id = id_value.as_str().ok_or("Trinket ID was not a string.")?.to_string();
        let mut chars = id.chars();
        let Some(last) = chars.next_back() else { return Err("Trinket ID is empty.".into()); };
        if chars.next_back() != Some('_') || !"CUREL".contains(last) {
            return Err(format!("\"{id}\" doesn't end in a recognized rarity suffix (_C/_U/_R/_E/_L)."));
        }
        let last_char_byte_at = id_value_at + 4 + id.len() - 1;
        self.data[last_char_byte_at] = letter as u8;
        self.changes.push(format!("Trinket #{}: rarity suffix changed to _{letter}", index));
        self.rescan_preserving_changes();
        Ok(self.snapshot())
    }

    fn remove_stone(&mut self, index: usize) -> Result<Snapshot, String> {
        let (Some(count_at), Some(size_at), Some(struct_size_at)) = (self.stone_count_at, self.stone_array_size_at, self.stone_struct_size_at) else { return Err("Stone array size metadata was not validated; removal is disabled.".into()); };
        if self.stones.len() <= 1 { return Err("At least one owned stone must remain.".into()); }
        let stone = self.stones.iter().find(|s| s.index == index).cloned().ok_or("Selected stone was not found.")?;
        let removed = stone.end - stone.start;
        if removed == 0 || removed > self.data.len() || get_u32(&self.data, size_at).unwrap_or(0) < removed as u32 { return Err("Stone record boundary failed validation.".into()); }
        let mut updated = self.data.clone();
        updated.drain(stone.start..stone.end);
        put_u32(&mut updated, count_at, (self.stones.len() - 1) as u32)?;
        let new_array_size = get_u32(&updated, size_at).unwrap_or(0) - removed as u32;
        let new_struct_size = get_u32(&updated, struct_size_at).unwrap_or(0).checked_sub(removed as u32).ok_or("Containing stone struct size underflow.")?;
        put_u32(&mut updated, size_at, new_array_size)?;
        put_u32(&mut updated, struct_size_at, new_struct_size)?;
        self.data = updated;
        self.changes.push(format!("Removed stone #{}: {} ({})", stone.index, stone.id, stone.stone_set));
        self.rescan_preserving_changes();
        Ok(self.snapshot())
    }

    fn undo(&mut self) -> Snapshot {
        self.data = self.original.clone();
        self.changes.clear();
        self.rescan_preserving_changes();
        self.snapshot()
    }

    fn save(&mut self) -> Result<Snapshot, String> {
        self.validate()?;
        let disk = fs::read(&self.path).map_err(|e| format!("Could not check the save before writing: {e}"))?;
        if disk != self.original { return Err("The save changed on disk since it was opened. Reopen the latest save before editing.".into()); }
        let stamp = Local::now().format("%Y%m%d_%H%M%S");
        let name = self.path.file_name().and_then(|n| n.to_str()).unwrap_or("save.sav");
        let mut backup = self.path.with_file_name(format!("{name}.bak_{stamp}"));
        let mut suffix = 1;
        while backup.exists() { backup = self.path.with_file_name(format!("{name}.bak_{stamp}_{suffix}")); suffix += 1; }
        fs::copy(&self.path, &backup).map_err(|e| format!("Could not create backup: {e}"))?;
        let temp = self.path.with_file_name(format!("{name}.{}.tmp", rand_guid()));
        if let Err(e) = fs::write(&temp, &self.data) { return Err(format!("Could not write temporary save: {e}")); }
        if fs::metadata(&temp).map_err(|e| e.to_string())?.len() as usize != self.data.len() { let _ = fs::remove_file(&temp); return Err("Temporary save size did not validate.".into()); }
        fs::remove_file(&self.path).map_err(|e| { let _ = fs::remove_file(&temp); format!("Could not replace the save: {e}") })?;
        if let Err(e) = fs::rename(&temp, &self.path) {
            let _ = fs::remove_file(&temp);
            if let Err(restore) = fs::copy(&backup, &self.path) { return Err(format!("Could not replace the save ({e}) and backup recovery also failed ({restore}). Original backup: {}", backup.display())); }
            return Err(format!("Could not replace the save: {e}. The original was restored from its backup."));
        }
        self.original = self.data.clone();
        self.changes.clear();
        self.backup_path = Some(backup);
        Ok(self.snapshot())
    }

    /// A manual, on-demand snapshot of whatever is currently on disk at self.path --
    /// independent of any pending in-memory edits or the automatic backup made by
    /// save(). This is the "create a safety backup" button: a deliberate checkpoint
    /// the user can restore to later if something (an edit, a bug, anything) goes wrong.
    fn create_backup(&self) -> Result<BackupInfo, String> {
        let name = self.path.file_name().and_then(|n| n.to_str()).unwrap_or("save.sav");
        let stamp = Local::now().format("%Y%m%d_%H%M%S");
        let mut backup = self.path.with_file_name(format!("{name}.bak_{stamp}"));
        let mut suffix = 1;
        while backup.exists() { backup = self.path.with_file_name(format!("{name}.bak_{stamp}_{suffix}")); suffix += 1; }
        fs::copy(&self.path, &backup).map_err(|e| format!("Could not create backup: {e}"))?;
        backup_info(&backup)
    }

    fn list_backups(&self) -> Result<Vec<BackupInfo>, String> {
        let name = self.path.file_name().and_then(|n| n.to_str()).unwrap_or("save.sav").to_string();
        let dir = self.path.parent().ok_or("Save has no parent directory.")?;
        let prefix = format!("{name}.bak_");
        let mut out = Vec::new();
        for entry in fs::read_dir(dir).map_err(|e| format!("Could not list backups: {e}"))? {
            let entry = entry.map_err(|e| e.to_string())?;
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.starts_with(&prefix) {
                out.push(backup_info(&entry.path())?);
            }
        }
        out.sort_by(|a, b| b.filename.cmp(&a.filename));
        Ok(out)
    }

    fn validate(&self) -> Result<(), String> {
        if !self.data.starts_with(MAGIC) { return Err("Save header changed unexpectedly.".into()); }
        if let (Some(count_at), Some(size_at), Some(data_at), Some(end)) = (self.stone_count_at, self.stone_array_size_at, self.stone_data_at, self.stone_end) {
            if get_u32(&self.data, count_at).unwrap_or(0) as usize != self.stones.len() || data_at + get_u32(&self.data, size_at).unwrap_or(0) as usize != end { return Err("Stone count and array size do not agree.".into()); }
        }
        for p in &self.properties { if p.value_at.is_some_and(|offset| offset >= self.data.len()) { return Err(format!("Field {} points outside the save.", p.name)); } }
        Ok(())
    }
}

#[derive(Default)]
struct AppState(Mutex<Option<LoadedSave>>);

#[tauri::command]
fn open_save(app: AppHandle, state: State<'_, AppState>) -> Result<Snapshot, String> {
    let file = app.dialog().file().add_filter("Serious Sam save", &["sav"]).blocking_pick_file().ok_or("No save selected.")?;
    let path = file.into_path().map_err(|e| format!("Invalid save path: {e}"))?;
    let loaded = LoadedSave::open(path)?;
    let snapshot = loaded.snapshot();
    *state.0.lock().map_err(|_| "Save state lock failed.")? = Some(loaded);
    Ok(snapshot)
}

#[tauri::command]
fn import_stone_catalog(app: AppHandle) -> Result<Vec<StoneObservation>, String> {
    let file = app.dialog().file().add_filter("Serious Sam save", &["sav"]).blocking_pick_file().ok_or("Import cancelled.")?;
    let path = file.into_path().map_err(|e| format!("Invalid save path: {e}"))?;
    LoadedSave::open(path)?.observations()
}

#[tauri::command]
fn import_exported_trinket(app: AppHandle) -> Result<StoneObservation, String> {
    let file = app.dialog().file().add_filter("Shatterverse trinket export", &["trinket"]).blocking_pick_file().ok_or("Import cancelled.")?;
    let path = file.into_path().map_err(|e| format!("Invalid trinket path: {e}"))?;
    let bytes = fs::read(path).map_err(|e| format!("Could not read trinket export: {e}"))?;
    observation_from_block(&bytes)
}

#[tauri::command]
fn export_trinket(app: AppHandle, state: State<'_, AppState>, index: usize) -> Result<(), String> {
    let bytes = {
        let guard = state.0.lock().map_err(|_| "Save state lock failed.")?;
        guard.as_ref().ok_or("Open a save first.")?.export_trinket(index)?
    };
    let file = app.dialog().file().add_filter("Trinket export", &["trinket"]).set_file_name("trinket.trinket").blocking_save_file().ok_or("Export cancelled.")?;
    let path = file.into_path().map_err(|e| format!("Invalid export path: {e}"))?;
    fs::write(&path, &bytes).map_err(|e| format!("Could not write export file: {e}"))?;
    Ok(())
}

#[tauri::command]
fn import_trinket(app: AppHandle, state: State<'_, AppState>) -> Result<Snapshot, String> {
    let file = app.dialog().file().add_filter("Trinket export", &["trinket"]).blocking_pick_file().ok_or("Import cancelled.")?;
    let path = file.into_path().map_err(|e| format!("Invalid import path: {e}"))?;
    let bytes = fs::read(&path).map_err(|e| format!("Could not read import file: {e}"))?;
    state.0.lock().map_err(|_| "Save state lock failed.")?.as_mut().ok_or("Open a save first.")?.import_trinket(bytes)
}

#[tauri::command]
fn open_save_path(state: State<'_, AppState>, path: String) -> Result<Snapshot, String> {
    let loaded = LoadedSave::open(PathBuf::from(path))?;
    let snapshot = loaded.snapshot();
    *state.0.lock().map_err(|_| "Save state lock failed.")? = Some(loaded);
    Ok(snapshot)
}

#[tauri::command]
fn import_trinket_path(state: State<'_, AppState>, path: String) -> Result<Snapshot, String> {
    let bytes = fs::read(&path).map_err(|e| format!("Could not read dropped file: {e}"))?;
    state.0.lock().map_err(|_| "Save state lock failed.")?.as_mut().ok_or("Open a save first.")?.import_trinket(bytes)
}

#[tauri::command]
fn stage_edits(state: State<'_, AppState>, edits: Vec<Edit>) -> Result<Snapshot, String> {
    state.0.lock().map_err(|_| "Save state lock failed.")?.as_mut().ok_or("Open a save first.")?.stage(edits)
}

#[tauri::command]
fn replace_stone(state: State<'_, AppState>, index: usize, template: Value) -> Result<Snapshot, String> {
    state.0.lock().map_err(|_| "Save state lock failed.")?.as_mut().ok_or("Open a save first.")?.replace_stone(index, template)
}

#[tauri::command]
fn add_stone(state: State<'_, AppState>, template_index: usize, observation: StoneObservation) -> Result<Snapshot, String> {
    state.0.lock().map_err(|_| "Save state lock failed.")?.as_mut().ok_or("Open a save first.")?.add_stone(template_index, observation)
}

#[tauri::command]
fn remove_stone(state: State<'_, AppState>, index: usize) -> Result<Snapshot, String> {
    state.0.lock().map_err(|_| "Save state lock failed.")?.as_mut().ok_or("Open a save first.")?.remove_stone(index)
}

#[tauri::command]
fn set_trinket_rarity(state: State<'_, AppState>, index: usize, letter: String) -> Result<Snapshot, String> {
    let letter = letter.chars().next().ok_or("Rarity letter is required.")?;
    state.0.lock().map_err(|_| "Save state lock failed.")?.as_mut().ok_or("Open a save first.")?.set_trinket_rarity(index, letter)
}

#[tauri::command]
fn undo(state: State<'_, AppState>) -> Result<Snapshot, String> {
    Ok(state.0.lock().map_err(|_| "Save state lock failed.")?.as_mut().ok_or("Open a save first.")?.undo())
}

#[tauri::command]
fn save_changes(state: State<'_, AppState>) -> Result<Snapshot, String> {
    state.0.lock().map_err(|_| "Save state lock failed.")?.as_mut().ok_or("Open a save first.")?.save()
}

#[tauri::command]
fn current_snapshot(state: State<'_, AppState>) -> Result<Option<Snapshot>, String> {
    Ok(state.0.lock().map_err(|_| "Save state lock failed.")?.as_ref().map(LoadedSave::snapshot))
}

#[tauri::command]
fn list_backups(state: State<'_, AppState>) -> Result<Vec<BackupInfo>, String> {
    state.0.lock().map_err(|_| "Save state lock failed.")?.as_ref().ok_or("Open a save first.")?.list_backups()
}

#[tauri::command]
fn create_backup(state: State<'_, AppState>) -> Result<BackupInfo, String> {
    state.0.lock().map_err(|_| "Save state lock failed.")?.as_ref().ok_or("Open a save first.")?.create_backup()
}

#[tauri::command]
fn restore_backup(state: State<'_, AppState>, filename: String) -> Result<Snapshot, String> {
    let mut guard = state.0.lock().map_err(|_| "Save state lock failed.")?;
    let current = guard.as_ref().ok_or("Open a save first.")?;
    let loaded = restore_backup_file(&current.path, &filename)?;
    let snapshot = loaded.snapshot();
    *guard = Some(loaded);
    Ok(snapshot)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![open_save, open_save_path, import_stone_catalog, import_exported_trinket, stage_edits, replace_stone, add_stone, remove_stone, export_trinket, set_trinket_rarity, undo, save_changes, current_snapshot, list_backups, create_backup, restore_backup])
        .run(tauri::generate_context!())
        .expect("error while running Shatterverse Save Editor");
}

fn ftag(name: &str) -> Vec<u8> {
    let mut out = ((name.len() + 1) as i32).to_le_bytes().to_vec();
    out.extend_from_slice(name.as_bytes()); out.push(0); out
}

fn find_all(data: &[u8], pattern: &[u8]) -> Vec<usize> {
    if pattern.is_empty() || pattern.len() > data.len() { return vec![]; }
    data.windows(pattern.len()).enumerate().filter_map(|(i, w)| (w == pattern).then_some(i)).collect()
}
fn find_bytes(data: &[u8], pattern: &[u8]) -> Option<usize> { data.windows(pattern.len()).position(|w| w == pattern) }
fn find_bytes_from(data: &[u8], pattern: &[u8], start: usize) -> Option<usize> { find_bytes_range(data, pattern, start, data.len()) }
fn find_bytes_range(data: &[u8], pattern: &[u8], start: usize, end: usize) -> Option<usize> {
    let hi = end.min(data.len()); if start >= hi || pattern.len() > hi - start { return None; }
    data[start..hi].windows(pattern.len()).position(|w| w == pattern).map(|n| n + start)
}
fn rfind_bytes_before(data: &[u8], pattern: &[u8], end: usize) -> Option<usize> {
    let hi = end.min(data.len()); if pattern.len() > hi { return None; }
    data[..hi].windows(pattern.len()).rposition(|w| w == pattern)
}
fn read_fstr(data: &[u8], at: usize) -> Option<(String, usize)> {
    let length = get_i32(data, at)?; let start = at.checked_add(4)?;
    if length > 0 { let end = start.checked_add(length as usize)?; let bytes = data.get(start..end)?; if bytes.last() != Some(&0) { return None; } return Some((String::from_utf8_lossy(&bytes[..bytes.len()-1]).to_string(), end)); }
    if length < 0 { let len = length.unsigned_abs() as usize * 2; let end = start.checked_add(len)?; let bytes = data.get(start..end)?; if bytes.len() < 2 { return None; } let units = bytes[..bytes.len()-2].chunks_exact(2).map(|c| u16::from_le_bytes([c[0],c[1]])).collect::<Vec<_>>(); return Some((String::from_utf16_lossy(&units), end)); }
    Some((String::new(), start))
}
fn parse_property(data: &[u8], at: usize) -> Option<Property> {
    let (name, after_name) = read_fstr(data, at)?;
    if name.is_empty() || name.len() > 160 || name.chars().any(char::is_control) { return None; }
    let (ty, after_type) = read_fstr(data, after_name)?;
    if !["BoolProperty", "ByteProperty", "IntProperty", "UInt32Property", "Int64Property", "FloatProperty", "NameProperty", "StrProperty", "ArrayProperty", "MapProperty", "StructProperty"].contains(&ty.as_str()) { return None; }
    let payload = after_type.checked_add(8)?; if payload >= data.len() { return None; }
    let mut value_at = None; let mut value = Value::Null; let mut editable = false;
    match ty.as_str() {
        "BoolProperty" => { value_at = Some(payload); value = json!(data[payload]); editable = data[payload] == 0 || data[payload] == 16; }
        "ByteProperty" => { let at = payload.checked_add(5)?; let n = *data.get(at)?; value_at = Some(at); value = json!(n); editable = n <= 1; }
        "IntProperty" => { let guid = *data.get(payload)?; if guid > 1 { return None; } let at = payload + 1 + if guid == 1 {16} else {0}; let n = get_i32(data, at)?; value_at=Some(at); value=json!(n); editable=true; }
        "UInt32Property" => { let guid=*data.get(payload)?; if guid>1{return None;} let at=payload+1+if guid==1{16}else{0};let n=get_u32(data,at)?;value_at=Some(at);value=json!(n);editable=true; }
        "Int64Property" => { let guid=*data.get(payload)?;if guid>1{return None;}let at=payload+1+if guid==1{16}else{0};let n=get_i64(data,at)?;value_at=Some(at);value=json!(n);editable=true; }
        "FloatProperty" => { let guid=*data.get(payload)?;if guid>1{return None;}let at=payload+1+if guid==1{16}else{0};let n=get_f32(data,at)?;value_at=Some(at);value=json!(n); }
        "NameProperty" | "StrProperty" => { let guid=*data.get(payload)?;if guid>1{return None;}let at=payload+1+if guid==1{16}else{0};let (s,_)=read_fstr(data,at)?;value_at=Some(at);value=json!(s); }
        "ArrayProperty" => { let count=get_u32(data,payload).unwrap_or(0); let size=get_i32(data,after_type).unwrap_or(0); value=json!({"kind":"Array","entries":count,"bytes":size.max(0)}); }
        "StructProperty" | "MapProperty" => { let size=get_i32(data,after_type).unwrap_or(0); value=json!({"kind":"Structure","bytes":size.max(0)}); }
        _ => {}
    }
    Some(Property { name, type_name: ty, tag_at: at, value_at, value, editable, scope: String::new(), note: String::new() })
}
fn tagged_value_range(data: &[u8], start: usize, end: usize, name: &str, ty: &str) -> Option<(Value, usize)> {
    let tag = [ftag(name), ftag(ty)].concat(); let at = find_bytes_range(data, &tag, start, end)?; let p = parse_property(data, at)?; Some((p.value, p.value_at?))
}
fn read_secondary_entries(data: &[u8], start: usize, end: usize) -> Result<BTreeMap<String, (i64, usize)>, String> {
    let end = end.min(data.len());
    let tag = [ftag("SecS"), ftag("MapProperty")].concat();
    let at = find_bytes_range(data, &tag, start, end).ok_or("Trinket is missing its bonus-stat map.")?;
    let (_, after_name) = read_fstr(data, at).ok_or("Invalid bonus-stat map name.")?;
    let (_, after_type) = read_fstr(data, after_name).ok_or("Invalid bonus-stat map type.")?;
    let (key_type, after_key_type) = read_fstr(data, after_type + 4).ok_or("Invalid bonus-stat key type.")?;
    if key_type != "NameProperty" { return Err("Unsupported bonus-stat map key type.".into()); }
    let (value_type, after_value_type) = read_fstr(data, after_key_type + 4).ok_or("Invalid bonus-stat value type.")?;
    if value_type != "IntProperty" { return Err("Unsupported bonus-stat map value type.".into()); }
    let count_at = after_value_type.checked_add(13).ok_or("Bonus-stat map offset overflow.")?;
    let count = get_u32(data, count_at).ok_or("Truncated bonus-stat map.")? as usize;
    if count > 128 { return Err("Bonus-stat map contains an implausible number of entries.".into()); }
    let mut cursor = count_at + 4;
    let mut values = BTreeMap::new();
    for _ in 0..count {
        let (key, after_key) = read_fstr(data, cursor).ok_or("Invalid bonus-stat key.")?;
        if key.is_empty() || key.chars().any(char::is_control) { return Err("Bonus-stat map contains an invalid key.".into()); }
        let value = get_i32(data, after_key).ok_or("Truncated bonus-stat value.")?;
        if values.insert(key, (i64::from(value), after_key)).is_some() { return Err("Bonus-stat map contains duplicate keys.".into()); }
        cursor = after_key + 4;
        if cursor > end { return Err("Bonus-stat map extends beyond the trinket record.".into()); }
    }
    Ok(values)
}
fn patch_secondary_map(block: &mut [u8], wanted: &BTreeMap<String, i64>) -> Result<(), String> {
    let current = read_secondary_entries(block, 0, block.len())?;
    if current.keys().ne(wanted.keys()) { return Err("The chosen structure template has a different bonus-stat layout than the imported observation.".into()); }
    for (key, (_, at)) in current {
        let value = wanted.get(&key).ok_or("Bonus-stat template mismatch.")?;
        put_i32(block, at, i32::try_from(*value).map_err(|_| format!("Bonus stat {key} is outside Int32 range."))?)?;
    }
    Ok(())
}
fn get_i32(data: &[u8], at: usize) -> Option<i32> { Some(i32::from_le_bytes(data.get(at..at+4)?.try_into().ok()?)) }
fn get_u32(data: &[u8], at: usize) -> Option<u32> { Some(u32::from_le_bytes(data.get(at..at+4)?.try_into().ok()?)) }
fn get_i64(data: &[u8], at: usize) -> Option<i64> { Some(i64::from_le_bytes(data.get(at..at+8)?.try_into().ok()?)) }
fn get_f32(data: &[u8], at: usize) -> Option<f32> { Some(f32::from_le_bytes(data.get(at..at+4)?.try_into().ok()?)) }
fn put_u32(data: &mut [u8], at: usize, n: u32) -> Result<(), String> { let out=data.get_mut(at..at+4).ok_or("Value offset is outside the save.")?;out.copy_from_slice(&n.to_le_bytes());Ok(()) }
fn put_i32(data: &mut [u8], at: usize, n: i32) -> Result<(), String> { let out=data.get_mut(at..at+4).ok_or("Value offset is outside the save.")?;out.copy_from_slice(&n.to_le_bytes());Ok(()) }
fn read_property_value(data: &[u8], p: &Property) -> Result<Value, String> {
    let at=p.value_at.ok_or("Property has no scalar value.")?;
    match p.type_name.as_str() { "BoolProperty"|"ByteProperty"=>Ok(json!(*data.get(at).ok_or("Offset outside file.")?)),"IntProperty"=>Ok(json!(get_i32(data,at).ok_or("Offset outside file.")?)),"UInt32Property"=>Ok(json!(get_u32(data,at).ok_or("Offset outside file.")?)),"Int64Property"=>Ok(json!(get_i64(data,at).ok_or("Offset outside file.")?)),"Map<IntProperty>"=>Ok(json!(get_i32(data,at).ok_or("Offset outside file.")?)),"Map<UInt32Property>"=>Ok(json!(get_u32(data,at).ok_or("Offset outside file.")?)),"Map<Int64Property>"=>Ok(json!(get_i64(data,at).ok_or("Offset outside file.")?)),_=>Ok(p.value.clone()) }
}
fn write_property(data: &mut [u8], p: &Property, value: &Value) -> Result<(), String> {
    let at=p.value_at.ok_or("Property is read-only.")?;
    match p.type_name.as_str() {
        "BoolProperty" => {let b=value.as_bool().ok_or("Expected true or false.")?;*data.get_mut(at).ok_or("Offset outside file.")?=if b{16}else{0};}
        "ByteProperty" => {let n=value.as_u64().filter(|n|*n<=1).ok_or("This byte accepts only 0 or 1.")?;*data.get_mut(at).ok_or("Offset outside file.")?=n as u8;}
        "IntProperty"|"Map<IntProperty>" => {let n=value.as_i64().and_then(|n|i32::try_from(n).ok()).ok_or("Value is outside the Int32 range.")?;if p.scope=="Currencies"&&n<1{return Err("Currency values must be at least 1; zero crashes the game while loading this save.".into());}put_i32(data,at,n)?;}
        "UInt32Property"|"Map<UInt32Property>" => {let n=value.as_u64().and_then(|n|u32::try_from(n).ok()).ok_or("Value is outside the UInt32 range.")?;put_u32(data,at,n)?;}
        "Int64Property"|"Map<Int64Property>" => {let n=value.as_i64().ok_or("Value is outside the Int64 range.")?;data.get_mut(at..at+8).ok_or("Offset outside file.")?.copy_from_slice(&n.to_le_bytes());}
        _=>return Err(format!("{} is read-only.",p.type_name)),
    }
    Ok(())
}
fn patch_i32_field(block: &mut [u8], name: &str, ty: &str, value: i32) -> Result<(), String> {
    let tag = [ftag(name), ftag(ty)].concat();
    let at = find_bytes(block, &tag).ok_or_else(|| format!("Template has no {name} field."))?;
    let payload = at + tag.len() + 8;
    let guid = *block.get(payload).ok_or("Invalid field tag.")?;
    let value_at = payload + 1 + if guid == 1 { 16 } else { 0 };
    put_i32(block, value_at, value)
}
fn patch_u32_field(block: &mut [u8], name: &str, ty: &str, value: u32) -> Result<(), String> {
    let tag = [ftag(name), ftag(ty)].concat();
    let at = find_bytes(block, &tag).ok_or_else(|| format!("Template has no {name} field."))?;
    let payload = at + tag.len() + 8;
    let guid = *block.get(payload).ok_or("Invalid field tag.")?;
    let value_at = payload + 1 + if guid == 1 { 16 } else { 0 };
    put_u32(block, value_at, value)
}
fn patch_byte_field(block: &mut [u8], name: &str, ty: &str, value: u8) -> Result<(), String> {
    let tag = [ftag(name), ftag(ty)].concat();
    let at = find_bytes(block, &tag).ok_or_else(|| format!("Template has no {name} field."))?;
    let value_at = at + tag.len() + 8 + 5;
    *block.get_mut(value_at).ok_or("Invalid field tag.")? = value;
    Ok(())
}
fn patch_fstring_field(block: &mut Vec<u8>, name: &str, ty: &str, replacement: &str) -> Result<(), String> {
    let tag=[ftag(name),ftag(ty)].concat();let at=find_bytes(block,&tag).ok_or_else(||format!("Template has no {name} field."))?;let size_at=at+tag.len();let payload=size_at+8;let guid=*block.get(payload).ok_or("Invalid field tag.")?;let value_at=payload+1+if guid==1{16}else{0};let (_,end)=read_fstr(block,value_at).ok_or("Invalid template string.")?;let mut encoded=(replacement.len() as i32+1).to_le_bytes().to_vec();encoded.extend_from_slice(replacement.as_bytes());encoded.push(0);let delta=encoded.len() as i64-(end-value_at) as i64;let old_size=get_i32(block,size_at).ok_or("Invalid field size.")?;let new_size=i32::try_from(old_size as i64+delta).map_err(|_|"Replacement string exceeds the supported field size.")?;block.splice(value_at..end,encoded);put_i32(block,size_at,new_size)?;Ok(())
}
fn backup_info(path: &std::path::Path) -> Result<BackupInfo, String> {
    let meta = fs::metadata(path).map_err(|e| format!("Could not read backup metadata: {e}"))?;
    let modified_at = meta.modified().ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| chrono::DateTime::<Local>::from(std::time::UNIX_EPOCH + d).format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default();
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    let label = filename.split(".bak_").nth(1).unwrap_or(&filename).to_string();
    Ok(BackupInfo { filename, label, size: meta.len(), modified_at })
}

/// Restores a chosen backup over the live save file. Backs up whatever is currently
/// on disk FIRST (so a bad restore is itself always recoverable), then copies the
/// backup into place and reloads it fresh.
fn restore_backup_file(path: &std::path::Path, filename: &str) -> Result<LoadedSave, String> {
    let dir = path.parent().ok_or("Save has no parent directory.")?;
    let backup_path = dir.join(filename);
    if !backup_path.exists() { return Err(format!("Backup {filename} was not found.")); }
    if path.exists() {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("save.sav");
        let stamp = Local::now().format("%Y%m%d_%H%M%S");
        let mut pre_restore = path.with_file_name(format!("{name}.bak_{stamp}_before_restore"));
        let mut suffix = 1;
        while pre_restore.exists() { pre_restore = path.with_file_name(format!("{name}.bak_{stamp}_before_restore_{suffix}")); suffix += 1; }
        fs::copy(path, &pre_restore).map_err(|e| format!("Could not back up the current save before restoring: {e}"))?;
    }
    fs::copy(&backup_path, path).map_err(|e| format!("Could not restore backup: {e}"))?;
    LoadedSave::open(path.to_path_buf())
}

fn rand_guid() -> u128 { use std::sync::atomic::{AtomicU64, Ordering}; use std::time::{SystemTime, UNIX_EPOCH}; static SEQUENCE: AtomicU64 = AtomicU64::new(0); let ticks=SystemTime::now().duration_since(UNIX_EPOCH).map(|d|d.as_nanos()).unwrap_or(0); let sequence=SEQUENCE.fetch_add(1, Ordering::Relaxed) as u128; ticks ^ ((std::process::id() as u128) << 64) ^ sequence.rotate_left(37) }

#[cfg(test)]
mod repro_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn codex_scope_labeling_works() {
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test_input.sav");
        let loaded = LoadedSave::open(src).expect("open");
        println!("warnings: {:?}", loaded.warnings);
        let unlock_count = loaded.properties.iter().filter(|p| p.scope.starts_with("Unlock: ")).count();
        let codex_count = loaded.properties.iter().filter(|p| p.scope.starts_with("Codex: ")).count();
        println!("Unlock: scoped properties = {unlock_count}");
        println!("Codex: scoped properties = {codex_count}");
        let sams: Vec<_> = loaded.properties.iter().filter(|p| p.scope.starts_with("Unlock: Sam_")).map(|p| (p.scope.clone(), p.name.clone(), p.value.clone())).collect();
        println!("Sam entries found: {:?}", sams);
        let sam_scopes: std::collections::HashSet<_> = loaded.properties.iter().filter(|p| p.scope.contains("Sam_01") || p.scope.contains("Raygun")).map(|p| p.scope.clone()).collect();
        println!("Any scope mentioning Sam_01 or Raygun: {:?}", sam_scopes);

        // Manually re-walk CodexEntries to inspect Sam_01's actual byte boundaries.
        let anchor = ftag("CodexEntries");
        let aidx = find_bytes(&loaded.data, &anchor).unwrap();
        let mut pos = aidx + anchor.len();
        let (_, next) = read_fstr(&loaded.data, pos).unwrap(); pos = next + 4;
        let (_, next) = read_fstr(&loaded.data, pos).unwrap(); pos = next + 4;
        let (_, next) = read_fstr(&loaded.data, pos).unwrap(); pos = next + 4;
        let (_, next) = read_fstr(&loaded.data, pos).unwrap(); pos = next;
        let declared_size = get_u32(&loaded.data, pos + 4).unwrap();
        let data_at = pos + 13;
        let array_end = data_at + declared_size as usize;
        let id_tag = [ftag("ID"), ftag("NameProperty")].concat();
        let starts: Vec<usize> = find_all(&loaded.data, &id_tag).into_iter().filter(|at| *at >= data_at && *at < array_end).collect();
        println!("manual: data_at={data_at} array_end={array_end} starts.len()={}", starts.len());
        for i in 0..starts.len() {
            let s = starts[i];
            let ne = if i+1 < starts.len() { starts[i+1] } else { array_end };
            if let Some((idv, _)) = tagged_value_range(&loaded.data, s, ne, "ID", "NameProperty") {
                let idname = idv.as_str().unwrap_or("").to_string();
                if idname == "Sam_01" || idname == "Raygun" {
                    println!("entry {i}: id={idname} start={s} next_start={ne} len={}", ne - s);
                    let cqp_tag = [ftag("CQP"), ftag("ArrayProperty")].concat();
                    println!("  cqp found at: {:?}", find_bytes_range(&loaded.data, &cqp_tag, s, ne));
                }
            }
        }
        assert!(unlock_count > 0, "no Unlock: scoped properties found at all");
    }

    #[test]
    fn rarity_swap_does_not_corrupt() {
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test_input.sav");
        let dst = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test_output_rarity.sav");
        fs::copy(&src, &dst).expect("copy input");

        let mut loaded = LoadedSave::open(dst.clone()).expect("open");
        assert!(!loaded.stones.is_empty(), "test save has no stones");
        let target = loaded.stones[0].clone();
        println!("before: id={} rank={}", target.id, target.rank);

        let new_letter = if target.id.ends_with('L') { 'C' } else { 'L' };
        loaded.set_trinket_rarity(target.index, new_letter).expect("set rarity");
        let after_id = loaded.stones.iter().find(|s| s.index == target.index).unwrap().id.clone();
        println!("after: id={}", after_id);
        assert_ne!(after_id, target.id);
        assert!(after_id.ends_with(new_letter));

        loaded.save().expect("save");
        let reopened = LoadedSave::open(dst.clone()).expect("reopen");
        let reopened_stone = reopened.stones.iter().find(|s| s.index == target.index).expect("stone still present");
        println!("reopened: id={}", reopened_stone.id);
        assert_eq!(reopened_stone.id, after_id);
        assert_eq!(reopened.stones.len(), loaded.stones.len());
    }
}
