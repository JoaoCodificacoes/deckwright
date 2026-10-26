use deckwright::{Limits, Package};
use std::io::{Cursor, Write};
fn fixture(extra: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (n,b) in [("[Content_Types].xml", b"<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"bin\" ContentType=\"application/octet-stream\"/><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/></Types>".as_slice())].into_iter().chain(extra.iter().copied()){w.start_file(n,zip::write::SimpleFileOptions::default()).unwrap();w.write_all(b).unwrap();}
    w.finish().unwrap().into_inner()
}
#[test]
fn preserves_unknown_xml_and_binary_and_is_deterministic() {
    let xml = b"<x:unknown xmlns:x=\"urn:test\">  untouched </x:unknown>";
    let bytes = fixture(&[("custom/data.xml", xml), ("media/blob.bin", &[0, 255, 1])]);
    let p = Package::from_bytes(bytes, Limits::default()).unwrap();
    assert_eq!(p.part("custom/data.xml").unwrap(), xml);
    let a = p.to_bytes().unwrap();
    assert_eq!(a, p.to_bytes().unwrap());
    let p = Package::from_bytes(a, Limits::default()).unwrap();
    assert_eq!(p.part("media/blob.bin").unwrap(), [0, 255, 1]);
}
#[test]
fn rejects_duplicate_traversal_and_limits() {
    assert!(
        Package::from_bytes(
            {
                let mut b = fixture(&[("a.xml", b"<a/>"), ("b.xml", b"<b/>")]);
                for i in 0..b.len() - 5 {
                    if &b[i..i + 5] == b"b.xml" {
                        b[i] = b'a';
                    }
                }
                b
            },
            Limits::default()
        )
        .is_err()
    );
    assert!(Package::from_bytes(fixture(&[("../a", b"x")]), Limits::default()).is_err());
    let limits = Limits {
        max_uncompressed_bytes: 5,
        ..Limits::default()
    };
    assert!(Package::from_bytes(fixture(&[]), limits).is_err());
}

#[test]
fn resolves_relative_targets_and_reports_external_without_fetching() {
    let rel=b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"r1\" Type=\"slide\" Target=\"../media/blob.bin\"/><Relationship Id=\"r2\" Type=\"link\" Target=\"https://invalid.example/never-fetch\" TargetMode=\"External\"/></Relationships>";
    let p = Package::from_bytes(
        fixture(&[
            ("ppt/slides/a.xml", b"<slide/>"),
            ("ppt/slides/_rels/a.xml.rels", rel),
            ("ppt/media/blob.bin", b"abc"),
        ]),
        Limits::default(),
    )
    .unwrap();
    assert_eq!(
        p.relationships()[0].resolved_target.as_deref(),
        Some("ppt/media/blob.bin")
    );
    assert_eq!(p.relationships()[1].target_mode, "External");
    assert_eq!(p.relationships()[1].resolved_target, None);
}
#[test]
fn rejects_missing_relationship_target() {
    let p=Package::from_bytes(fixture(&[("_rels/.rels",b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"r1\" Type=\"slide\" Target=\"missing.xml\"/></Relationships>")]),Limits::default());
    assert!(matches!(p, Err(deckwright::Error::Relationship { .. })));
}
#[test]
fn rejects_dtd_malformed_xml_and_depth() {
    for xml in [
        b"<!DOCTYPE a [<!ENTITY b SYSTEM 'file:///etc/passwd'>]><a/>".as_slice(),
        b"<a><b></a>",
    ] {
        assert!(Package::from_bytes(fixture(&[("a.xml", xml)]), Limits::default()).is_err());
    }
    let limits = Limits {
        max_xml_depth: 2,
        ..Limits::default()
    };
    assert!(Package::from_bytes(fixture(&[("a.xml", b"<a><b><c/></b></a>")]), limits).is_err());
}
#[test]
fn inventories_presentation_order_effects_and_unknown_extensions() {
    let p=Package::from_bytes(fixture(&[("ppt/presentation.xml",b"<p:presentation xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><p:sldIdLst><p:sldId r:id=\"r1\"/></p:sldIdLst><p:sldSz cx=\"100\" cy=\"200\"/></p:presentation>"),("ppt/_rels/presentation.xml.rels",b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"r1\" Type=\"slide\" Target=\"slides/slide9.xml\"/></Relationships>"),("ppt/slides/slide9.xml",b"<p:sld xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:sp/><evil:sp xmlns:evil=\"urn:unknown\"/><evil:cTn xmlns:evil=\"urn:unknown\" presetClass=\"evil\"/><p:timing><p:cTn id=\"7\" presetClass=\"entr\"/><p:spTgt spid=\"4\"/></p:timing><p:extLst><p:unknown/></p:extLst></p:sld>")]),Limits::default()).unwrap();
    let i = p.inventory();
    assert_eq!(i["slides"][0]["part"], "ppt/slides/slide9.xml");
    assert_eq!(i["totals"]["effects"], 1);
    assert_eq!(i["slides"][0]["counts"]["sp"], 1);
    assert_eq!(i["slides"][0]["timing_targets"][0]["spid"], "4");
}

#[test]
fn unicode_targets_and_alternate_prefixes() {
    let p=Package::from_bytes(fixture(&[("_rels/.rels", "<r:Relationships xmlns:r=\"http://schemas.openxmlformats.org/package/2006/relationships\"><r:Relationship Id=\"r1\" Type=\"x\" Target=\"média.bin\"/><r:Relationship Id=\"r2\" Type=\"x\" Target=\"m%C3%A9dia.bin\"/></r:Relationships>".as_bytes()),("média.bin",b"abc")]),Limits::default()).unwrap();
    assert_eq!(
        p.relationships()[0].resolved_target.as_deref(),
        Some("média.bin")
    );
    assert_eq!(
        p.relationships()[1].resolved_target.as_deref(),
        Some("média.bin")
    );
}

#[test]
fn rejects_extra_document_text_and_duplicate_expanded_attributes() {
    for xml in [
        b"<a/>unexpected".as_slice(),
        b"<a xmlns:x=\"urn:x\" xmlns:y=\"urn:x\" x:id=\"1\" y:id=\"2\"/>",
    ] {
        assert!(Package::from_bytes(fixture(&[("a.xml", xml)]), Limits::default()).is_err());
    }
}

#[test]
fn rejects_each_resource_limit_and_crc_corruption() {
    let bytes = fixture(&[("a.xml", b"<a/>")]);
    for limits in [
        Limits {
            max_input_bytes: 1,
            ..Limits::default()
        },
        Limits {
            max_part_bytes: 3,
            ..Limits::default()
        },
        Limits {
            max_parts: 1,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            Package::from_bytes(bytes.clone(), limits),
            Err(deckwright::Error::Limit(_))
        ));
    }
    let mut bytes = fixture(&[("blob.bin", b"UNIQUE_PAYLOAD")]);
    let pos = bytes.windows(4).rposition(|w| w == b"PK\x01\x02").unwrap();
    bytes[pos + 16] ^= 1;
    assert!(Package::from_bytes(bytes, Limits::default()).is_err());
}
