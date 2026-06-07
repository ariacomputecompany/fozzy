use super::path::normalize_zip_entry_rel_path;
use super::*;
use ::zip::ZipWriter;
use ::zip::write::SimpleFileOptions;
use std::fs::File;
use std::io::Write;

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            let lsb = crc & 1;
            crc >>= 1;
            if lsb != 0 {
                crc ^= 0xEDB8_8320;
            }
        }
    }
    !crc
}

fn build_zip_with_raw_entries(entries: &[(&[u8], &[u8])]) -> Vec<u8> {
    let mut out = Vec::<u8>::new();
    let mut central = Vec::<u8>::new();
    let mut offsets = Vec::<u32>::new();

    for (name, payload) in entries {
        let offset = out.len() as u32;
        offsets.push(offset);
        let crc = crc32(payload);
        let name_len = name.len() as u16;
        let size = payload.len() as u32;

        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(payload);
    }

    let cd_offset = out.len() as u32;
    for ((name, payload), offset) in entries.iter().zip(offsets.iter().copied()) {
        let crc = crc32(payload);
        let name_len = name.len() as u16;
        let size = payload.len() as u32;
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&name_len.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
    }
    let cd_size = central.len() as u32;
    out.extend_from_slice(&central);

    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[cfg(unix)]
#[test]
fn import_rejects_symlink_target_overwrite() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!("fozzy-corpus-symlink-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("root");
    let zip_path = root.join("in.zip");

    {
        let file = File::create(&zip_path).expect("zip create");
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default();
        zip.start_file("payload.bin", opts).expect("start");
        zip.write_all(b"evil").expect("write");
        zip.finish().expect("finish");
    }

    let out = root.join("out");
    std::fs::create_dir_all(&out).expect("out");
    let victim = root.join("victim.bin");
    std::fs::write(&victim, b"safe").expect("victim");
    symlink(&victim, out.join("payload.bin")).expect("symlink");

    let err = import_zip(&zip_path, &out).expect_err("must fail");
    assert!(err.to_string().contains("symlinked output file"));
    assert_eq!(std::fs::read(&victim).expect("victim read"), b"safe");
}

#[cfg(unix)]
#[test]
fn import_failure_atomic_on_symlink_error() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "fozzy-corpus-atomic-symlink-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("root");
    let zip_path = root.join("in.zip");

    {
        let file = File::create(&zip_path).expect("zip create");
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default();
        zip.start_file("good-1.bin", opts).expect("start 1");
        zip.write_all(b"one").expect("write 1");
        zip.start_file("good-2.bin", opts).expect("start 2");
        zip.write_all(b"two").expect("write 2");
        zip.start_file("bad.bin", opts).expect("start bad");
        zip.write_all(b"bad").expect("write bad");
        zip.finish().expect("finish");
    }

    let out = root.join("out");
    std::fs::create_dir_all(&out).expect("out");
    let victim = root.join("victim.bin");
    std::fs::write(&victim, b"safe").expect("victim");
    symlink(&victim, out.join("bad.bin")).expect("symlink");

    let err = import_zip(&zip_path, &out).expect_err("must fail");
    assert!(err.to_string().contains("symlinked output file"));
    assert_eq!(std::fs::read(&victim).expect("victim read"), b"safe");
    assert!(
        !out.join("good-1.bin").exists(),
        "good-1 should not be written"
    );
    assert!(
        !out.join("good-2.bin").exists(),
        "good-2 should not be written"
    );
}

#[test]
fn normalize_rejects_windows_style_unsafe_paths() {
    for bad in [
        r"..\\evil_win.bin",
        r"C:\evil_drive.bin",
        "C:evil_drive.bin",
        r"\\server\share\evil_unc.bin",
        "//server/share/evil_unc.bin",
    ] {
        let err =
            normalize_zip_entry_rel_path(bad).expect_err("must reject windows-style unsafe path");
        assert!(
            err.to_string()
                .contains("unsafe archive entry path rejected")
        );
    }
}

#[test]
fn normalize_rejects_special_unsafe_filenames() {
    for bad in [
        "\u{0001}.bin",
        "\u{0000}TRUNC.bin",
        "CON",
        "aux.txt",
        "name-with-trailing-dot.",
        "name-with-trailing-space ",
        "bad:name.bin",
        "bad*name.bin",
        "bad?name.bin",
    ] {
        let err =
            normalize_zip_entry_rel_path(bad).expect_err("must reject unsafe special filename");
        assert!(
            err.to_string()
                .contains("unsafe archive entry path rejected")
        );
    }
}

#[test]
fn import_rejects_duplicate_entry_aliases() {
    let root =
        std::env::temp_dir().join(format!("fozzy-corpus-dup-alias-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("root");
    let zip_path = root.join("in.zip");

    {
        let file = File::create(&zip_path).expect("zip create");
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default();
        zip.start_file("dup.bin", opts).expect("start 1");
        zip.write_all(b"first").expect("write 1");
        zip.start_file("./dup.bin", opts).expect("start 2");
        zip.write_all(b"second").expect("write 2");
        zip.finish().expect("finish");
    }

    let out = root.join("out");
    std::fs::create_dir_all(&out).expect("out");

    let err = import_zip(&zip_path, &out).expect_err("must reject alias duplicates");
    assert!(
        err.to_string()
            .contains("duplicate output file in archive is not allowed")
    );
    assert!(
        !out.join("dup.bin").exists(),
        "duplicate rejection should be failure-atomic"
    );
}

#[test]
fn import_rejects_case_insensitive_duplicate_entry_names() {
    let root = std::env::temp_dir().join(format!("fozzy-corpus-dup-case-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("root");
    let zip_path = root.join("in.zip");

    {
        let file = File::create(&zip_path).expect("zip create");
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default();
        zip.start_file("dup.bin", opts).expect("start 1");
        zip.write_all(b"first").expect("write 1");
        zip.start_file("DUP.BIN", opts).expect("start 2");
        zip.write_all(b"second").expect("write 2");
        zip.finish().expect("finish");
    }

    let out = root.join("out");
    std::fs::create_dir_all(&out).expect("out");

    let err = import_zip(&zip_path, &out).expect_err("must reject case-insensitive duplicates");
    assert!(
        err.to_string()
            .contains("duplicate output file in archive is not allowed")
    );
    assert!(
        !out.join("dup.bin").exists(),
        "duplicate rejection should be failure-atomic"
    );
    assert!(
        !out.join("DUP.BIN").exists(),
        "duplicate rejection should be failure-atomic"
    );
}

#[test]
fn import_rejects_overwrite_of_existing_file() {
    let root =
        std::env::temp_dir().join(format!("fozzy-corpus-overwrite-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("root");
    let zip_path = root.join("in.zip");

    {
        let file = File::create(&zip_path).expect("zip create");
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default();
        zip.start_file("dup.bin", opts).expect("start");
        zip.write_all(b"new").expect("write");
        zip.finish().expect("finish");
    }

    let out = root.join("out");
    std::fs::create_dir_all(&out).expect("out");
    std::fs::write(out.join("dup.bin"), b"old").expect("seed existing");

    let err = import_zip(&zip_path, &out).expect_err("must reject overwrite");
    assert!(
        err.to_string()
            .contains("refusing to overwrite existing output file")
    );
    assert_eq!(std::fs::read(out.join("dup.bin")).expect("read"), b"old");
}

#[test]
fn import_rejects_nul_in_raw_entry_name() {
    let root = std::env::temp_dir().join(format!("fozzy-corpus-nul-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("root");
    let zip_path = root.join("in.zip");

    {
        let file = File::create(&zip_path).expect("zip create");
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default();
        zip.start_file("bad\0name.bin", opts).expect("start");
        zip.write_all(b"payload").expect("write");
        zip.finish().expect("finish");
    }

    let out = root.join("out");
    std::fs::create_dir_all(&out).expect("out");

    let err = import_zip(&zip_path, &out).expect_err("must reject nul entry names");
    assert!(
        err.to_string()
            .contains("unsafe archive entry path rejected")
    );
    assert!(!out.join("bad").exists(), "must not write truncated output");
}

#[test]
fn import_rejects_duplicate_entry_names_from_raw_headers() {
    let root = std::env::temp_dir().join(format!("fozzy-corpus-rawdup-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("root");
    let zip_path = root.join("dup.zip");
    let out = root.join("out");
    std::fs::create_dir_all(&out).expect("out");

    let zip = build_zip_with_raw_entries(&[(b"dup.bin", b"FIRST"), (b"dup.bin", b"SECOND")]);
    std::fs::write(&zip_path, zip).expect("zip write");

    let err = import_zip(&zip_path, &out).expect_err("must reject duplicates");
    assert!(
        err.to_string()
            .contains("duplicate output file in archive is not allowed")
    );
    assert!(!out.join("dup.bin").exists(), "should fail before writes");
}

#[test]
fn import_rejects_nul_collision_aliases_from_raw_headers() {
    let root =
        std::env::temp_dir().join(format!("fozzy-corpus-rawnuldup-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("root");
    let zip_path = root.join("dup.zip");
    let out = root.join("out");
    std::fs::create_dir_all(&out).expect("out");

    let zip = build_zip_with_raw_entries(&[(b"bad\0suffix.bin", b"FIRST"), (b"bad", b"SECOND")]);
    std::fs::write(&zip_path, zip).expect("zip write");

    let err = import_zip(&zip_path, &out).expect_err("must reject nul-collision aliases");
    assert!(
        err.to_string()
            .contains("unsafe archive entry path rejected")
    );
    assert!(!out.join("bad").exists(), "should fail before writes");
}

#[test]
fn minimize_deduplicates_and_canonicalizes_corpus_files() {
    let root = std::env::temp_dir().join(format!("fozzy-corpus-minimize-{}", uuid::Uuid::new_v4()));
    let corpus = root.join("corpus");
    std::fs::create_dir_all(&corpus).expect("corpus");
    std::fs::write(corpus.join("a.bin"), b"alpha").expect("alpha 1");
    std::fs::write(corpus.join("b.bin"), b"beta").expect("beta");
    std::fs::write(corpus.join("nested-name.bin"), b"alpha").expect("alpha 2");

    let out = minimize_corpus(&corpus, None).expect("minimize");
    assert_eq!(out.get("filesBefore").and_then(|v| v.as_u64()), Some(3));
    assert_eq!(out.get("filesAfter").and_then(|v| v.as_u64()), Some(2));
    assert_eq!(
        out.get("duplicatesRemoved").and_then(|v| v.as_u64()),
        Some(1)
    );

    let mut names = std::fs::read_dir(&corpus)
        .expect("read dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names,
        vec![
            format!("input-{}.bin", blake3::hash(b"alpha").to_hex()),
            format!("input-{}.bin", blake3::hash(b"beta").to_hex())
        ]
    );
}

#[test]
fn minimize_rejects_empty_corpus_directory() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-corpus-minimize-empty-{}",
        uuid::Uuid::new_v4()
    ));
    let corpus = root.join("corpus");
    std::fs::create_dir_all(&corpus).expect("corpus");

    let err = minimize_corpus(&corpus, None).expect_err("must reject empty corpus");
    assert!(
        err.to_string()
            .contains("corpus directory has no files to minimize")
    );
}

#[cfg(unix)]
#[test]
fn export_rejects_symlinked_output_file() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "fozzy-corpus-export-symlink-file-{}",
        uuid::Uuid::new_v4()
    ));
    let corpus = root.join("corpus");
    std::fs::create_dir_all(&corpus).expect("corpus");
    std::fs::write(corpus.join("input.bin"), b"data").expect("input");

    let victim = root.join("victim.zip");
    std::fs::write(&victim, b"safe").expect("victim");
    let out = root.join("out.zip");
    symlink(&victim, &out).expect("symlink");

    let err = export_zip(&corpus, &out).expect_err("must reject symlinked output file");
    assert!(err.to_string().contains("symlinked output file"));
    assert_eq!(std::fs::read(&victim).expect("victim"), b"safe");
}

#[cfg(unix)]
#[test]
fn export_rejects_symlinked_parent_path() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "fozzy-corpus-export-symlink-parent-{}",
        uuid::Uuid::new_v4()
    ));
    let corpus = root.join("corpus");
    std::fs::create_dir_all(&corpus).expect("corpus");
    std::fs::write(corpus.join("input.bin"), b"data").expect("input");

    let real_dir = root.join("real");
    std::fs::create_dir_all(&real_dir).expect("real");
    let link_parent = root.join("linkp");
    symlink(&real_dir, &link_parent).expect("symlink parent");
    let out = link_parent.join("out.zip");

    let err = export_zip(&corpus, &out).expect_err("must reject symlink parent");
    assert!(err.to_string().contains("symlinked output path"));
    assert!(!out.exists(), "must not create zip via symlinked parent");
}

#[test]
fn export_rejects_missing_source_directory() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-corpus-export-missing-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("root");
    let out = root.join("out.zip");
    let src = root.join("does-not-exist");

    let err = export_zip(&src, &out).expect_err("must reject missing source");
    assert!(err.to_string().contains("corpus directory not found"));
    assert!(!out.exists(), "must not create zip for missing source");
}

#[test]
fn export_rejects_empty_source_directory() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-corpus-export-empty-{}",
        uuid::Uuid::new_v4()
    ));
    let src = root.join("corpus");
    std::fs::create_dir_all(&src).expect("src");
    let out = root.join("out.zip");

    let err = export_zip(&src, &out).expect_err("must reject empty source");
    assert!(
        err.to_string()
            .contains("corpus directory has no files to export")
    );
    assert!(!out.exists(), "must not create zip for empty source");
}

#[test]
fn export_rejects_file_source_path() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-corpus-export-file-source-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("root");
    let src = root.join("source.bin");
    std::fs::write(&src, b"payload").expect("source");
    let out = root.join("out.zip");

    let err = export_zip(&src, &out).expect_err("must reject non-directory source");
    assert!(
        err.to_string()
            .contains("corpus export source is not a directory")
    );
    assert!(
        !out.exists(),
        "must not create zip for non-directory source"
    );
}

#[cfg(unix)]
#[test]
fn export_failure_does_not_clobber_existing_output_file() {
    use std::os::unix::fs::PermissionsExt;

    let root = std::env::temp_dir().join(format!(
        "fozzy-corpus-export-clobber-{}",
        uuid::Uuid::new_v4()
    ));
    let src = root.join("corpus");
    std::fs::create_dir_all(&src).expect("src");
    let unreadable = src.join("secret.bin");
    std::fs::write(&unreadable, b"secret").expect("file");
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000)).expect("chmod");

    let out = root.join("out.zip");
    std::fs::write(&out, b"KEEP").expect("seed output");

    let err = export_zip(&src, &out).expect_err("must fail on unreadable source");
    assert!(err.to_string().contains("Permission denied"));
    assert_eq!(std::fs::read(&out).expect("out read"), b"KEEP");

    // cleanup for tempdir removal
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o600))
        .expect("restore chmod");
}
