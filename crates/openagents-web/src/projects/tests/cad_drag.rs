//! A real local CAD page for pointer and touch checks with isolated accounts.
use super::*;

#[tokio::test]
#[ignore = "serves a local CAD page; needs the native and browser CAD builds"]
async fn serve_cad_drag() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    HOST_OVERRIDE.set(format!("127.0.0.1:{port}")).unwrap();
    let world = world_at(
        port,
        Options {
            cad: Some(std::env::var_os("OA_CAD_BROWSER").unwrap().into()),
            ..Options::default()
        },
    )
    .await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let scope = crate::cad_web::Scope::personal(&account_owner_of(&world, "octo-local")).unwrap();
    let engine = crate::cad_web::engine().expect("set OPENAGENTS_CAD_ENGINE");
    let result=engine.run(&world.store,&scope,"cad_from_json",&json!({"doc":"bracket","document":{
        "kind":"part.v1","id":"bracket","title":"Fan bracket","material":"Aluminum","features":[
            {"id":"body","op":"solid.box","params":{"length":50,"width":30,"height":8,"center":[0,0,4]}},
            {"id":"notch","op":"solid.box","params":{"length":40,"width":30,"height":12,"center":[10,10,6],"operation":"cut"}}
        ]}})).await;
    assert_eq!(result["saved"], true, "{result}");
    // The actual authenticated route refuses a stale page and bad CSRF.
    let page = browser.get(&world, "/cad/bracket").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    let csrf = page
        .body
        .split("data-cad-csrf=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    for (token, version, status) in [
        (csrf, "0", StatusCode::CONFLICT),
        ("wrong", "1", StatusCode::FORBIDDEN),
    ] {
        let refused = browser
            .post(
                &world,
                "/cad/bracket/pull",
                &[
                    ("csrf", token),
                    ("version", version),
                    ("part", "bracket"),
                    ("face", "body:+z"),
                    ("delta", "2"),
                ],
            )
            .await;
        assert_eq!(refused.status, status, "{}", refused.body);
    }
    let out = std::path::PathBuf::from(std::env::var_os("OA_CAD_CAPTURE").unwrap());
    std::fs::write(
        out,
        serde_json::to_vec(
            &json!({"url":format!("{}/cad/bracket",world.origin()),"cookies":browser.0}),
        )
        .unwrap(),
    )
    .unwrap();
    axum::serve(listener, world.site).await.unwrap();
}

#[tokio::test]
#[ignore = "serves a local configurator; needs native and browser CAD builds"]
async fn serve_cad_configurator() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    HOST_OVERRIDE.set(format!("127.0.0.1:{port}")).unwrap();
    let world = world_at(
        port,
        Options {
            cad: Some(std::env::var_os("OA_CAD_BROWSER").unwrap().into()),
            ..Options::default()
        },
    )
    .await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let scope = crate::cad_web::Scope::personal(&account_owner_of(&world, "octo-local")).unwrap();
    let result=crate::cad_web::engine().unwrap().run(&world.store,&scope,"cad_from_json",&json!({"doc":"bracket","document":{
        "kind":"part.v1","id":"bracket","title":"Fan bracket","material":"Aluminum",
        "parameters":[{"name":"hole_spacing","expression":"24 mm"},{"name":"thickness","expression":"6 mm"}],
        "features":[
            {"id":"body","op":"solid.box","params":{"length":60,"width":40,"height":"thickness","center":[0,0,"thickness / 2"]}},
            {"id":"notch","op":"solid.box","params":{"length":40,"width":20,"height":30,"center":[15,15,13],"operation":"cut"}},
            {"id":"left-hole","op":"solid.cylinder","params":{"radius":2,"height":30,"base":["-hole_spacing / 2",-8,-2],"operation":"cut"}},
            {"id":"right-hole","op":"solid.cylinder","params":{"radius":2,"height":30,"base":["hole_spacing / 2",-8,-2],"operation":"cut"}}
        ]}})).await;
    assert_eq!(result["saved"], true, "{result}");
    let out = std::path::PathBuf::from(std::env::var_os("OA_CAD_CAPTURE").unwrap());
    std::fs::write(out,serde_json::to_vec(&json!({"url":format!("{}/cad/bracket/configurator",world.origin()),"cookies":browser.0})).unwrap()).unwrap();
    axum::serve(listener, world.site).await.unwrap();
}

#[tokio::test]
#[ignore = "serves a local model-upload page; needs native and browser CAD builds"]
async fn serve_cad_imports() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    HOST_OVERRIDE.set(format!("127.0.0.1:{port}")).unwrap();
    let world = world_at(
        port,
        Options {
            cad: Some(std::env::var_os("OA_CAD_BROWSER").unwrap().into()),
            ..Options::default()
        },
    )
    .await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let scope = crate::cad_web::Scope::personal(&account_owner_of(&world, "octo-local")).unwrap();
    let engine = crate::cad_web::engine().unwrap();
    let result=engine.run(&world.store,&scope,"cad_from_json",&json!({"doc":"source","document":{"kind":"part.v1","id":"box","title":"Source box","features":[{"id":"body","op":"solid.box","params":{"length":10,"width":12,"height":14,"center":[0,0,7]}}]}})).await;
    assert_eq!(result["saved"], true, "{result}");
    let result = engine
        .run(
            &world.store,
            &scope,
            "cad_export",
            &json!({"doc":"source","formats":["step","stl"]}),
        )
        .await;
    assert!(result["files"].is_array(), "{result}");
    let out = std::path::PathBuf::from(std::env::var_os("OA_CAD_CAPTURE").unwrap());
    let root = out.parent().unwrap();
    for format in ["step", "stl"] {
        let bytes = crate::cad_web::read_file(
            &world.store,
            &scope,
            "source",
            1,
            &format!("out/source-v1.{format}"),
        )
        .await
        .unwrap();
        std::fs::write(root.join(format!("cad-import-source.{format}")), bytes).unwrap();
    }
    std::fs::write(
        out,
        serde_json::to_vec(
            &json!({"url":format!("{}/cad/source",world.origin()),"cookies":browser.0}),
        )
        .unwrap(),
    )
    .unwrap();
    axum::serve(listener, world.site).await.unwrap();
}

#[tokio::test]
#[ignore = "serves a local manufacturing card; needs a native CAD build"]
async fn serve_cad_manufacture() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    HOST_OVERRIDE.set(format!("127.0.0.1:{port}")).unwrap();
    let world = world_at(
        port,
        Options {
            cad: Some(std::env::var_os("OA_CAD_BROWSER").unwrap().into()),
            ..Options::default()
        },
    )
    .await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let scope = crate::cad_web::Scope::personal(&account_owner_of(&world, "octo-local")).unwrap();
    let engine = crate::cad_web::engine().unwrap();
    let made=engine.run(&world.store,&scope,"cad_from_json",&json!({"doc":"bracket","document":{"kind":"part.v1","id":"bracket","title":"Print bracket","features":[
        {"id":"post","op":"solid.box","params":{"length":2,"width":10,"height":20,"center":[-9,0,10]}},
        {"id":"beam","op":"solid.box","params":{"length":20,"width":10,"height":0.3,"center":[0,0,19.85],"operation":"join"}}
    ]}})).await;
    assert_eq!(made["saved"], true, "{made}");
    let checked = engine
        .run(
            &world.store,
            &scope,
            "cad_manufacture",
            &json!({"doc":"bracket"}),
        )
        .await;
    assert_eq!(checked["manufacturing"]["passed"], false, "{checked}");
    let markup = crate::cad_lab::card_markup(CHAT, &checked["card"])
        .into_string()
        .replace(
            &format!("/chat/{CHAT}/cad/file?doc=bracket&amp;v=1&amp;name="),
            "/cad/bracket/file?v=1&amp;name=",
        );
    let site=world.site.clone().route("/cad/bracket/check-card",axum::routing::get(move|headers:HeaderMap|{
        let markup=markup.clone();async move {
            let mut response=crate::ui_page::UiPage::new("Print checks").head(maud::html!{link rel="stylesheet" href=(crate::cad_lab::ASSET_CSS);script src=(crate::cad_lab::ASSET_JS) defer {}}).content(maud::PreEscaped(markup)).respond(&headers);
            response.headers_mut().insert(header::CONTENT_SECURITY_POLICY,axum::http::HeaderValue::from_static(crate::pages::cad::CAD_POLICY));response
        }
    }));
    let out = std::path::PathBuf::from(std::env::var_os("OA_CAD_CAPTURE").unwrap());
    std::fs::write(out,serde_json::to_vec(&json!({"url":format!("{}/cad/bracket/check-card",world.origin()),"doc_url":format!("{}/cad/bracket?check=print",world.origin()),"cookies":browser.0})).unwrap()).unwrap();
    axum::serve(listener, site).await.unwrap();
}

#[tokio::test]
#[ignore = "serves isolated CAD handoffs; needs native CAD, Robot Lab and Everglade builds"]
async fn serve_cad_handoff() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    HOST_OVERRIDE.set(format!("127.0.0.1:{port}")).unwrap();
    let world = world_at(
        port,
        Options {
            cad: Some(std::env::var_os("OA_CAD_BROWSER").unwrap().into()),
            everglade: Some(std::env::var_os("OA_VERSE_BROWSER").unwrap().into()),
            ..Options::default()
        },
    )
    .await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let mut other = Browser::default();
    other.sign_in(&world, "quiet-local").await;
    let scope = crate::cad_web::Scope::personal(&account_owner_of(&world, "octo-local")).unwrap();
    let engine = crate::cad_web::engine().unwrap();
    let made=engine.run(&world.store,&scope,"cad_from_json",&json!({"doc":"robot","document":{"kind":"assembly.v1","id":"robot","title":"Hinged box","parts":{
        "base":{"kind":"part.v1","id":"base","features":[{"id":"body","op":"solid.box","params":{"length":600,"width":400,"height":200,"center":[0,0,100]}}]},
        "lid":{"kind":"part.v1","id":"lid","features":[{"id":"body","op":"solid.box","params":{"length":600,"width":400,"height":40,"center":[0,0,240]}}]}},
        "instances":[{"id":"base","part":"base"},{"id":"lid","part":"lid","parent":"base"}],
        "joints":[{"id":"hinge","type":"revolute","parent":"base","child":"lid","origin":[-300,0,220],"axis":[0,1,0],"limits":[[0,90]],"value":0}]}})).await;
    assert_eq!(made["saved"], true, "{made}");
    let robot = engine
        .run(
            &world.store,
            &scope,
            "cad_robot",
            &json!({"doc":"robot","version":1}),
        )
        .await;
    assert!(robot["robot"]["href"].is_string(), "{robot}");
    let placed = engine
        .run(
            &world.store,
            &scope,
            "cad_place",
            &json!({"doc":"robot","version":1}),
        )
        .await;
    assert!(placed["verse"]["href"].is_string(), "{placed}");
    std::fs::write(std::env::var_os("OA_CAD_CAPTURE").unwrap(),serde_json::to_vec(&json!({"url":format!("{}/cad/robot?v=1",world.origin()),"origin":world.origin(),"robot":robot["robot"]["href"],"verse":placed["verse"]["href"],"cookies":browser.0,"other_cookies":other.0})).unwrap()).unwrap();
    axum::serve(listener, world.site).await.unwrap();
}

#[tokio::test]
#[ignore = "serves an isolated CAD shop and signed test invoices; needs a native CAD build"]
async fn serve_cad_sales() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    HOST_OVERRIDE.set(format!("127.0.0.1:{port}")).unwrap();
    let wallet = crate::cad_web::fake_sale_wallet().await;
    let world = world_at(
        port,
        Options {
            cad: Some(std::env::var_os("OA_CAD_BROWSER").unwrap().into()),
            ..Options::default()
        },
    )
    .await;
    let mut seller = Browser::default();
    seller.sign_in(&world, "octo-local").await;
    let mut buyer = Browser::default();
    buyer.sign_in(&world, "quiet-local").await;
    let scope = crate::cad_web::Scope::personal(&account_owner_of(&world, "octo-local")).unwrap();
    let engine = crate::cad_web::engine().unwrap();
    let made=engine.run(&world.store,&scope,"cad_new_part",&json!({"doc":"bracket","part":"plate","material":"PLA","features":[{"id":"body","op":"solid.box","params":{"length":30,"width":20,"height":3}}]})).await;
    assert_eq!(made["saved"], true, "{made}");
    std::fs::write(std::env::var_os("OA_CAD_CAPTURE").unwrap(),serde_json::to_vec(&json!({"url":format!("{}/cad/bracket/configurator?v=1",world.origin()),"origin":world.origin(),"seller_cookies":seller.0,"buyer_cookies":buyer.0,"wallet":wallet.endpoint})).unwrap()).unwrap();
    axum::serve(listener, world.site).await.unwrap();
}
