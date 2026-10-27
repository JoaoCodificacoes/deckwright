use deckwright::{builder::compile_slide, model::Slide, units::Emu};

#[test]
fn rejects_invalid_supported_ooxml_attribute_domains() {
    use serde_json::{Value, json};
    fn merge(target: &mut Value, patch: Value) {
        if let (Some(target), Some(patch)) = (target.as_object_mut(), patch.as_object()) {
            for (key, value) in patch {
                merge(target.entry(key).or_insert(Value::Null), value.clone());
            }
        } else {
            *target = patch;
        }
    }
    let baseline: Value = serde_json::from_str(include_str!(
        "../../../fixtures/generated/authored-slide.json"
    ))
    .unwrap();
    let color = json!({"type":"rgb","value":"112233"});
    let reference = json!({"index":"1","color":color});
    let style = json!({"line":reference,"fill":reference,"effect":reference,"font":{"index":"minor","color":color}});
    let shadow = json!({"blur":0,"distance":0,"direction":0,"alignment":"ctr","rotate_with_shape":true,"color":color});
    let mut cases = vec![
        ("wrap", json!({"text":{"body":{"wrap":"banana"}}})),
        ("anchor", json!({"text":{"body":{"anchor":"banana"}}})),
        ("inset", json!({"text":{"body":{"left":2147483648_i64}}})),
        (
            "alignment",
            json!({"text":{"paragraphs":[{"properties":{"alignment":"banana"}}]}}),
        ),
        (
            "spacing",
            json!({"text":{"paragraphs":[{"properties":{"space_before":-1}}]}}),
        ),
        (
            "spacing",
            json!({"text":{"paragraphs":[{"properties":{"space_after":158401}}]}}),
        ),
        (
            "font size",
            json!({"text":{"paragraphs":[{"runs":[{"text":"x","style":{"size":99}}]}]}}),
        ),
        (
            "font size",
            json!({"text":{"paragraphs":[{"runs":[{"text":"x","style":{"size":400001}}]}]}}),
        ),
        (
            "field id",
            json!({"text":{"paragraphs":[{"runs":[{"text":"x","field":{"id":"not-a-guid","kind":"slidenum"}}]}]}}),
        ),
        ("line width", json!({"properties":{"line":{"width":-1}}})),
        (
            "line width",
            json!({"properties":{"line":{"width":20116801}}}),
        ),
        (
            "coordinate",
            json!({"properties":{"transform":{"x":27273042316901_i64}}}),
        ),
        (
            "extent",
            json!({"properties":{"transform":{"width":2147483648_i64}}}),
        ),
        (
            "child coordinate",
            json!({"properties":{"transform":{"child":{"x":0,"y":0,"width":1,"height":1}}}}),
        ),
        ("placeholder type", json!({"placeholder":{"kind":"banana"}})),
        (
            "placeholder size",
            json!({"placeholder":{"kind":"body","size":"banana"}}),
        ),
    ];
    for field in ["line", "fill", "effect", "font"] {
        let mut value = style.clone();
        value[field]["index"] = json!("banana");
        cases.push(("style index", json!({"style":value})));
    }
    for (field, value) in [
        ("blur", json!(-1)),
        ("distance", json!(2147483648_i64)),
        ("direction", json!(-1)),
        ("direction", json!(21600000)),
        ("alignment", json!("banana")),
    ] {
        let mut s = shadow.clone();
        s[field] = value;
        cases.push(("shadow", json!({"properties":{"effects":[s]}})));
    }
    for (label, patch) in cases {
        let mut value = baseline.clone();
        merge(&mut value["objects"][0]["kind"], patch);
        let slide: Slide = serde_json::from_value(value).unwrap();
        let error = compile_slide(&slide).expect_err(label).to_string();
        assert!(error.contains("heading"), "{label}: {error}");
    }
}

#[test]
fn accepts_supported_ooxml_boundary_values() {
    use serde_json::json;
    let color = json!({"type":"rgb","value":"112233","alpha":100000});
    let reference = json!({"index":"4294967295","color":color});
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/generated/authored-slide.json"
    ))
    .unwrap();
    let kind = &mut value["objects"][0]["kind"];
    kind["placeholder"] = json!({"kind":"clipArt","index":4294967295_u32,"size":"full"});
    kind["style"] = json!({"line":reference,"fill":reference,"effect":reference,"font":{"index":"none","color":color}});
    kind["properties"]["transform"] = json!({"x":-27273042329600_i64,"y":27273042316900_i64,"width":2147483647,"height":0,"rotation":-2147483648_i64});
    kind["properties"]["line"] = json!({"width":20116800});
    kind["properties"]["effects"] = json!([{"blur":2147483647,"distance":0,"direction":21599999,"alignment":"br","rotate_with_shape":false,"color":color}]);
    kind["text"]["body"] =
        json!({"wrap":"none","anchor":"b","left":-2147483648_i64,"right":2147483647});
    kind["text"]["paragraphs"] = json!([{"properties":{"alignment":"thaiDist","space_before":0,"space_after":158400},"runs":[{"text":"field","field":{"id":"{01234567-89AB-CDEF-0123-456789ABCDEF}","kind":"slidenum"},"style":{"size":100}},{"text":"large","style":{"size":400000}}]}]);
    assert!(compile_slide(&serde_json::from_value(value).unwrap()).is_ok());
}

#[test]
fn emu_conversion_rounds_at_explicit_boundary() {
    assert_eq!(Emu::from_inches(1.0).unwrap().0, 914400);
    assert_eq!(Emu::from_points(1.0).unwrap().0, 12700);
    assert_eq!(Emu::from_inches(-0.5).unwrap().0, -457200);
    assert!(Emu::from_inches(f64::NAN).is_err());
    assert!(Emu::from_inches(f64::INFINITY).is_err());
    assert!(Emu::from_inches(f64::MAX).is_err());
}

#[test]
fn native_authoring_mutates_only_named_text_and_geometry() {
    let mut slide: Slide = serde_json::from_str(include_str!(
        "../../../fixtures/generated/authored-slide.json"
    ))
    .unwrap();
    let mut sibling = slide.objects[0].clone();
    sibling.key = "sibling".into();
    sibling.id = 3;
    sibling.name = "Sibling".into();
    slide.objects.push(sibling);
    slide.set_text("sibling", "Retained sibling").unwrap();
    let before = compile_slide(&slide).unwrap();
    slide.set_text("heading", "Changed & editable").unwrap();
    slide.translate("heading", Emu(12700), Emu(-12700)).unwrap();
    let after = compile_slide(&slide).unwrap();
    let xml = String::from_utf8(after.clone()).unwrap();
    assert!(xml.contains("Changed &amp; editable"));
    assert!(!xml.contains(">Hello<"));
    assert!(xml.contains("<a:off x=\"12800\" y=\"-12500\"/>"));
    assert_ne!(before, after);
    let before_xml = String::from_utf8(before).unwrap();
    assert_eq!(
        before_xml.rsplit("<p:sp>").next(),
        xml.rsplit("<p:sp>").next()
    );
    assert!(slide.set_text("unknown", "oops").is_err());
}

#[test]
fn invalid_authored_keys_are_contextual_errors() {
    let mut slide: Slide = serde_json::from_str(include_str!(
        "../../../fixtures/generated/authored-slide.json"
    ))
    .unwrap();
    slide.objects.push(slide.objects[0].clone());
    assert!(
        compile_slide(&slide)
            .unwrap_err()
            .to_string()
            .contains("heading")
    );
}

#[test]
fn invalid_color_and_forbidden_xml_text_are_rejected() {
    let source: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/generated/authored-slide.json"
    ))
    .unwrap();
    let mut value = source.clone();
    value["objects"][0]["kind"]["text"]["paragraphs"][0]["runs"][0]["style"]["color"]["value"] =
        serde_json::json!("not-a-color");
    assert!(compile_slide(&serde_json::from_value(value).unwrap()).is_err());
    let mut slide: Slide = serde_json::from_value(source).unwrap();
    slide.set_text("heading", "invalid\u{1}text").unwrap();
    assert!(compile_slide(&slide).is_err());
}

#[test]
fn compiles_groups_crop_connections_inheritance_and_schema_order() {
    let slide: Slide = serde_json::from_value(serde_json::json!({"objects":[
        {"key":"group","id":2,"name":"Group","kind":{"type":"group","transform":{"x":10,"y":20,"width":400,"height":600,"rotation":90000,"child":{"x":100,"y":200,"width":200,"height":300}},"children":[
            {"key":"label","id":3,"name":"Label","morph_name":"!!label","kind":{"type":"shape","placeholder":{"kind":"body","index":10},"properties":{"transform":{"x":100,"y":200,"width":80,"height":90}},"text":{"list_style":true,"paragraphs":[{"properties":{"alignment":"ctr","space_after":400},"runs":[{"text":"A ","style":{"font":"Aptos","size":1400,"bold":true}},{"text":"B"}]}]}}},
            {"key":"picture","id":4,"name":"Picture","kind":{"type":"picture","relationship":"rId3","lock_aspect":true,"crop":{"left":1000,"top":2000,"right":3000,"bottom":4000},"properties":{"transform":{"x":180,"y":200,"width":50,"height":70},"geometry":{"preset":"rect"}}}},
            {"key":"connector","id":5,"name":"Connector","kind":{"type":"connector","start":{"object_id":3,"index":1},"end":{"object_id":4,"index":2},"properties":{"transform":{"x":180,"y":200,"width":0,"height":70},"geometry":{"preset":"line"},"line":{"width":12700,"fill":{"type":"solid","color":{"type":"rgb","value":"112233"}}}}}}
        ]}}
    ]})).unwrap();
    let bytes = compile_slide(&slide).unwrap();
    let xml = String::from_utf8(bytes.clone()).unwrap();
    assert!(xml.contains("<a:chOff x=\"100\" y=\"200\"/><a:chExt cx=\"200\" cy=\"300\"/>"));
    assert!(xml.contains("<a:srcRect l=\"1000\" t=\"2000\" r=\"3000\" b=\"4000\"/>"));
    assert!(xml.contains("<a:stCxn id=\"3\" idx=\"1\"/><a:endCxn id=\"4\" idx=\"2\"/>"));
    assert!(xml.contains("name=\"!!label\""));
    assert!(xml.contains("key=\"label\""));
    assert!(xml.contains("<a:r><a:t xml:space=\"preserve\">B</a:t></a:r>")); // inherits all run styles
    assert!(xml.find("<p:sp>").unwrap() < xml.find("<p:pic>").unwrap());
    assert!(xml.find("<p:pic>").unwrap() < xml.find("<p:cxnSp>").unwrap());
    // Parse the emitted XML independently, checking direct child order.
    let mut reader = quick_xml::Reader::from_reader(bytes.as_slice());
    let mut stack = Vec::<String>::new();
    let mut picture_children = Vec::new();
    loop {
        use quick_xml::events::Event;
        match reader.read_event().unwrap() {
            Event::Start(e) => {
                let name = String::from_utf8(e.name().as_ref().to_vec()).unwrap();
                if stack.last().map(String::as_str) == Some("p:pic") {
                    picture_children.push(name.clone());
                }
                stack.push(name);
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    assert_eq!(picture_children, ["p:nvPicPr", "p:blipFill", "p:spPr"]);
}

#[test]
fn rejects_duplicate_ids_dangling_connectors_and_invalid_geometry() {
    use deckwright::model::*;
    let mut slide: Slide = serde_json::from_str(include_str!(
        "../../../fixtures/generated/authored-slide.json"
    ))
    .unwrap();
    let mut second = slide.objects[0].clone();
    second.key = "second".into();
    slide.objects.push(second);
    assert!(
        compile_slide(&slide)
            .unwrap_err()
            .to_string()
            .contains("ID")
    );
    slide.objects.pop();
    slide.objects[0].kind = ObjectKind::Connector {
        properties: Properties::default(),
        style: None,
        start: Some(Connection {
            object_id: 99,
            index: 0,
        }),
        end: None,
    };
    assert!(
        compile_slide(&slide)
            .unwrap_err()
            .to_string()
            .contains("endpoint")
    );
    slide.objects[0].kind = ObjectKind::Shape {
        textbox: false,
        lock_no_group: false,
        placeholder: None,
        properties: Properties {
            geometry: Some(Geometry {
                preset: "unsupported".into(),
                adjustments: vec![],
            }),
            ..Properties::default()
        },
        style: None,
        text: None,
    };
    assert!(
        compile_slide(&slide)
            .unwrap_err()
            .to_string()
            .contains("unsupported geometry")
    );
}

#[test]
fn translation_overflow_is_atomic_and_rich_text_requires_explicit_edits() {
    use deckwright::model::*;
    let mut slide: Slide = serde_json::from_str(include_str!(
        "../../../fixtures/generated/authored-slide.json"
    ))
    .unwrap();
    let before = compile_slide(&slide).unwrap();
    assert!(slide.translate("heading", Emu(i64::MAX), Emu(10)).is_err());
    assert_eq!(before, compile_slide(&slide).unwrap());
    if let ObjectKind::Shape {
        text: Some(text), ..
    } = &mut slide.objects[0].kind
    {
        text.paragraphs[0].runs.push(TextRun::default());
    }
    assert!(slide.set_text("heading", "would erase styling").is_err());
    let mut json = serde_json::to_value(&slide).unwrap();
    json["objects"][0]["kind"]["properties"]["invented"] = serde_json::json!(1);
    assert!(serde_json::from_value::<Slide>(json).is_err());
}

#[test]
fn authored_package_edit_preserves_unrelated_parts_and_rejects_atomically() {
    use deckwright::{Limits, Package};
    use std::{
        collections::{BTreeMap, BTreeSet},
        io::{Cursor, Write},
    };
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name,data) in [
        ("[Content_Types].xml", "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"bin\" ContentType=\"application/octet-stream\"/></Types>".as_bytes()),
        ("slide.xml", b"<old/>"), ("untouched.xml", b"<u xmlns='urn:opaque'>  spaces  </u>"), ("media.bin", &[0,255,12])
    ] { zip.start_file(name,zip::write::SimpleFileOptions::default()).unwrap(); zip.write_all(data).unwrap(); }
    let package =
        Package::from_bytes(zip.finish().unwrap().into_inner(), Limits::default()).unwrap();
    let slide: Slide = serde_json::from_str(include_str!(
        "../../../fixtures/generated/authored-slide.json"
    ))
    .unwrap();
    let bytes = compile_slide(&slide).unwrap();
    let edited = package
        .edited(
            BTreeMap::from([("slide.xml".into(), bytes.clone())]),
            &BTreeSet::new(),
            Limits::default(),
        )
        .unwrap();
    assert_eq!(edited.part("slide.xml").unwrap(), bytes);
    for part in ["untouched.xml", "media.bin", "[Content_Types].xml"] {
        assert_eq!(package.part(part), edited.part(part));
    }
    assert!(
        package
            .edited(
                BTreeMap::from([("slide.xml".into(), b"<broken>".to_vec())]),
                &BTreeSet::new(),
                Limits::default()
            )
            .is_err()
    );
    assert_eq!(package.part("slide.xml"), Some(b"<old/>".as_slice()));
    assert_eq!(edited.to_bytes().unwrap(), edited.to_bytes().unwrap());
}
