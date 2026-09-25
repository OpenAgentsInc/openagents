use super::*;

/// An `.npy` file as `numpy.save` writes it.
pub(crate) fn npy(descr: &str, shape: &[usize], data: &[u8]) -> Vec<u8> {
    let shape_text = match shape {
        [one] => format!("({one},)"),
        dims => format!(
            "({})",
            dims.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    let mut header =
        format!("{{'descr': '{descr}', 'fortran_order': False, 'shape': {shape_text}, }}");
    while (10 + header.len() + 1) % 64 != 0 {
        header.push(' ');
    }
    header.push('\n');
    let mut out = npy::MAGIC.to_vec();
    out.extend([1, 0]);
    out.extend(u16::try_from(header.len()).unwrap().to_le_bytes());
    out.extend(header.as_bytes());
    out.extend(data);
    out
}

pub(crate) fn f8(values: &[f64]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// A ZIP archive of stored members, as `numpy.savez` writes one.
fn stored_zip(members: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut directory = Vec::new();
    for (name, data) in members {
        let offset = u32::try_from(out.len()).unwrap();
        let size = u32::try_from(data.len()).unwrap();
        let name_len = u16::try_from(name.len()).unwrap();
        out.extend(b"PK\x03\x04");
        out.extend([20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend(size.to_le_bytes());
        out.extend(size.to_le_bytes());
        out.extend(name_len.to_le_bytes());
        out.extend([0, 0]);
        out.extend(name.as_bytes());
        out.extend(data);
        directory.extend(b"PK\x01\x02");
        directory.extend([20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        directory.extend(size.to_le_bytes());
        directory.extend(size.to_le_bytes());
        directory.extend(name_len.to_le_bytes());
        directory.extend([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        directory.extend(offset.to_le_bytes());
        directory.extend(name.as_bytes());
    }
    let start = u32::try_from(out.len()).unwrap();
    let count = u16::try_from(members.len()).unwrap();
    let length = u32::try_from(directory.len()).unwrap();
    out.extend(&directory);
    out.extend(b"PK\x05\x06");
    out.extend([0, 0, 0, 0]);
    out.extend(count.to_le_bytes());
    out.extend(count.to_le_bytes());
    out.extend(length.to_le_bytes());
    out.extend(start.to_le_bytes());
    out.extend([0, 0]);
    out
}

fn workspace(files: &[(&str, Vec<u8>)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, bytes) in files {
        let at = dir.path().join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, bytes).unwrap();
    }
    dir
}

fn one(profile: &Profile, path: &str) -> FileProfile {
    profile
        .files
        .iter()
        .find(|f| f.path == path)
        .unwrap_or_else(|| panic!("no profile of {path}: {profile:#?}"))
        .clone()
}

#[test]
fn an_npy_matrix_shows_zero_rows_norms_and_nan() {
    let mut values = Vec::new();
    for row in 0..6 {
        let v = f64::from(row);
        values.extend([v, v + 1.0, 2.0]);
    }
    // Rows 2 and 4 are all zero; row 5 holds a NaN.
    values[6..9].copy_from_slice(&[0.0, 0.0, 0.0]);
    values[12..15].copy_from_slice(&[0.0, 0.0, 0.0]);
    values[16] = f64::NAN;
    let dir = workspace(&[("data/emb.npy", npy("<f8", &[6, 3], &f8(&values)))]);
    let profile = profile(dir.path(), &Params::default());
    let file = one(&profile, "data/emb.npy");
    assert_eq!(file.kind, Kind::Npy);
    assert!(file.whole);
    assert!(
        file.text.contains("dtype float64, shape (6, 3)"),
        "{}",
        file.text
    );
    assert!(
        file.findings
            .iter()
            .any(|f| f.starts_with("2 all-zero rows (rows 2, 4)")),
        "{:?}",
        file.findings
    );
    assert!(file.findings.iter().any(|f| f == "1 NaN values"));
    assert!(file.text.contains("Row L2 norms: min 0"), "{}", file.text);
    assert!(file.text.contains("Head (first 3 rows)"), "{}", file.text);
}

#[test]
fn npy_dtypes_and_byte_orders_read() {
    let big: Vec<u8> = [1.5f32, -2.0]
        .iter()
        .flat_map(|v| v.to_be_bytes())
        .collect();
    let header = npy::header(&npy(">f4", &[2], &big)).unwrap();
    assert_eq!(header.shape, vec![2]);
    let (values, rows) = npy::values(&npy(">f4", &[2], &big), &header).unwrap();
    assert_eq!((values, rows), (vec![1.5, -2.0], 2));
    let ints: Vec<u8> = [3i64, -4].iter().flat_map(|v| v.to_le_bytes()).collect();
    let bytes = npy("<i8", &[2, 1], &ints);
    let (values, _) = npy::values(&bytes, &npy::header(&bytes).unwrap()).unwrap();
    assert_eq!(values, vec![3.0, -4.0]);
    // Half precision: 1.0 is 0x3c00.
    let bytes = npy("<f2", &[1], &[0x00, 0x3c]);
    let (values, _) = npy::values(&bytes, &npy::header(&bytes).unwrap()).unwrap();
    assert_eq!(values, vec![1.0]);
    // A string dtype reads as its header only.
    let bytes = npy("<U3", &[1], &[0; 12]);
    let dir = workspace(&[("labels.npy", bytes)]);
    let file = one(&profile(dir.path(), &Params::default()), "labels.npy");
    assert!(file.text.contains("Values not read"), "{}", file.text);
}

#[test]
fn a_stored_npz_profiles_each_member() {
    let a = npy("<f8", &[2, 2], &f8(&[1.0, 2.0, 1.0, 2.0]));
    let archive = stored_zip(&[("a.npy", a)]);
    let dir = workspace(&[("arrays.npz", archive)]);
    let file = one(&profile(dir.path(), &Params::default()), "arrays.npz");
    assert!(
        file.text.contains("Member a.npy: dtype float64"),
        "{}",
        file.text
    );
    assert!(
        file.findings
            .iter()
            .any(|f| f == "1 duplicate rows in a.npy"),
        "{:?}",
        file.findings
    );
}

#[test]
fn a_csv_shows_its_columns_empty_fields_and_duplicates() {
    let csv = "id,score,label\n1,0.5,a\n2,,b\n3,1.5,\"b, c\"\n3,1.5,\"b, c\"\n4,2.5\n";
    let dir = workspace(&[("inputs/rows.csv", csv.as_bytes().to_vec())]);
    let file = one(&profile(dir.path(), &Params::default()), "inputs/rows.csv");
    assert_eq!(file.kind, Kind::Csv);
    assert!(
        file.text.contains("5 rows × 3 columns, with a header row"),
        "{}",
        file.text
    );
    for want in [
        "1 rows with a field count other than 3 (lines 6)",
        "1 duplicate rows",
        "1 empty fields in 1 columns",
    ] {
        assert!(
            file.findings.iter().any(|f| f == want),
            "{want}: {:?}",
            file.findings
        );
    }
    assert!(
        file.text.contains("Column \"score\": numeric"),
        "{}",
        file.text
    );
    assert!(file.text.contains("\"b, c\" ×2"), "{}", file.text);
}

#[test]
fn a_numeric_table_is_a_matrix_too() {
    let tsv = "x\ty\n0\t0\n3\t4\n";
    let dir = workspace(&[("points.tsv", tsv.as_bytes().to_vec())]);
    let file = one(&profile(dir.path(), &Params::default()), "points.tsv");
    assert!(
        file.findings
            .iter()
            .any(|f| f.starts_with("1 all-zero rows")),
        "{:?}",
        file.findings
    );
    assert!(
        file.text.contains("Row L2 norms: min 0, median 5"),
        "{}",
        file.text
    );
}

#[test]
fn json_and_json_lines_show_keys_nulls_and_bad_lines() {
    let jsonl = "{\"a\": 1, \"b\": \"x\"}\n{\"a\": null}\nnot json\n{\"a\": 3, \"b\": \"\"}\n";
    let json_doc = "{\"version\": 2, \"events\": [{\"t\": 1}, {\"t\": 2}, {\"t\": null}]}";
    let dir = workspace(&[
        ("events.jsonl", jsonl.as_bytes().to_vec()),
        ("config/events.json", json_doc.as_bytes().to_vec()),
    ]);
    let profile = profile(dir.path(), &Params::default());
    let lines = one(&profile, "events.jsonl");
    for want in [
        "1 lines that don't parse as JSON (lines 3)",
        "1 null values",
        "1 missing keys",
        "1 empty fields in 1 columns",
    ] {
        assert!(
            lines.findings.iter().any(|f| f == want),
            "{want}: {:?}",
            lines.findings
        );
    }
    let doc = one(&profile, "config/events.json");
    assert!(doc.text.contains("an object with 2 keys"), "{}", doc.text);
    assert!(
        doc.text.contains("Under \"events\" (3 items):"),
        "{}",
        doc.text
    );
    assert!(
        doc.findings
            .iter()
            .any(|f| f == "1 null values under \"events\""),
        "{:?}",
        doc.findings
    );
}

#[test]
fn a_log_counts_levels_and_dated_lines() {
    let log = "2026-09-25 10:00:01 INFO start\n2026-09-25 10:00:02 ERROR boom\n\n\
               2026-09-25 10:00:02 ERROR boom\n";
    let dir = workspace(&[("logs/app.log", log.as_bytes().to_vec())]);
    let file = one(&profile(dir.path(), &Params::default()), "logs/app.log");
    assert!(
        file.text.contains("Level words: ERROR 2, INFO 1"),
        "{}",
        file.text
    );
    assert!(file.text.contains("duplicate lines: 1"), "{}", file.text);
    assert!(
        file.text.contains("Lines that start with a date: 3"),
        "{}",
        file.text
    );
}

#[test]
fn code_configuration_and_unknown_files_are_not_data() {
    let dir = workspace(&[
        ("main.py", b"print(1)\n".to_vec()),
        ("package.json", b"{\"name\": \"x\"}".to_vec()),
        ("requirements.txt", b"numpy\n".to_vec()),
        ("README.txt", b"read me\n".to_vec()),
        (".hidden/data.csv", b"a,b\n1,2\n".to_vec()),
        ("image.png", b"\x89PNG".to_vec()),
        ("model.pkl", b"\x80\x04".to_vec()),
        ("table.parquet", b"PAR1....PAR1".to_vec()),
        // No extension, but the content is a CSV.
        ("measurements", b"a,b\n1,2\n3,4\n5,6\n".to_vec()),
    ]);
    let profile = profile(dir.path(), &Params::default());
    let paths: Vec<&str> = profile.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, vec!["measurements", "table.parquet"]);
    assert!(
        one(&profile, "table.parquet")
            .text
            .contains("no Parquet reader")
    );
    assert_eq!(
        profile.skipped,
        vec![(
            "model.pkl".to_string(),
            "no reader for .pkl in this build".to_string()
        )]
    );
}

#[test]
fn every_bound_holds() {
    let rows: String = (0..2_000).map(|i| format!("{i},{}\n", i * 2)).collect();
    let dir = workspace(&[
        ("a.csv", rows.as_bytes().to_vec()),
        ("b.csv", rows.as_bytes().to_vec()),
        ("c.csv", rows.as_bytes().to_vec()),
    ]);
    let params = Params {
        max_files: 2,
        read_bytes: 1_000,
        chars: 300,
        ..Params::default()
    };
    let profile = profile(dir.path(), &params);
    assert_eq!(profile.files.len(), 2);
    assert!(profile.files.iter().all(|f| !f.whole));
    assert!(
        one(&profile, "a.csv")
            .text
            .contains("first 1,000 bytes read")
    );
    assert_eq!(
        profile.skipped,
        vec![("c.csv".to_string(), "over the 2-file bound".to_string())]
    );
    for (label, text) in profile.items(&params) {
        assert!(label.starts_with(LABEL));
        assert!(text.chars().count() <= 300 + 60, "{text}");
    }
    assert!(Params::default().validate().is_ok());
    assert!(
        Params {
            chars: 10,
            ..Params::default()
        }
        .validate()
        .is_err()
    );
}

#[test]
fn numbers_read_as_a_person_writes_them() {
    assert_eq!(num(0.0), "0");
    assert_eq!(num(12.0), "12");
    assert_eq!(num(0.123_456), "0.1235");
    assert_eq!(num(1234.5678), "1235");
    assert_eq!(num(1240.4), "1240");
    assert_eq!(num(1e-7), "1.000e-7");
    assert_eq!(num(f64::NAN), "NaN");
}

#[test]
fn shared_values_across_tables_are_found_after_normalizing() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = String::from("id,ssn,city\n");
    let mut b = String::from("key,social,tier\n");
    for i in 0..120 {
        a.push_str(&format!(
            "A{i},{:03}-{:02}-{:04},Springfield\n",
            100 + i,
            i % 90,
            1000 + i
        ));
        // Half of b's identifiers match a's, written without dashes.
        let n = if i % 2 == 0 { i } else { 500 + i };
        b.push_str(&format!(
            "B{i},{:03}{:02}{:04},gold\n",
            100 + n,
            n % 90,
            1000 + n
        ));
    }
    std::fs::write(dir.path().join("a.csv"), a).unwrap();
    std::fs::write(dir.path().join("b.csv"), b).unwrap();
    let profile = super::profile(dir.path(), &super::Params::default());
    let shared = profile.shared.clone().expect("the pass reports the pair");
    assert!(
        shared.contains("a.csv `ssn` and b.csv `social`: 60 values in common"),
        "{shared}"
    );
    // The low-cardinality columns and the unrelated record ids aren't pairs.
    assert!(
        !shared.contains("city") && !shared.contains("tier") && !shared.contains("`id`"),
        "{shared}"
    );
    let items = profile.items(&super::Params::default());
    assert!(items.iter().any(|(label, _)| label == super::shared::LABEL));
}

#[test]
fn one_table_has_no_shared_values_item() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = String::from("id,ssn\n");
    for i in 0..100 {
        a.push_str(&format!("A{i},{:09}\n", 100_000_000 + i));
    }
    std::fs::write(dir.path().join("a.csv"), a).unwrap();
    assert!(
        super::profile(dir.path(), &super::Params::default())
            .shared
            .is_none()
    );
}

/// Profiles the folder `DATA_PROFILE_DIR` names and prints the shared-values
/// item: `DATA_PROFILE_DIR=… cargo test -p coder-one shared_values_on_a_folder -- --ignored --nocapture`.
#[test]
#[ignore = "reads a folder named by DATA_PROFILE_DIR"]
fn shared_values_on_a_folder() {
    let dir = std::env::var("DATA_PROFILE_DIR").expect("DATA_PROFILE_DIR");
    let profile = super::profile(std::path::Path::new(&dir), &super::Params::default());
    println!("{} ms\n{}", profile.ms, profile.shared.unwrap_or_default());
}
