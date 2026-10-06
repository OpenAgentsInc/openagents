use coder_new::{
    App, Mode, Screen,
    credentials::Imported,
    jev_plugin::{DEFAULT_ENDPOINT, GATEWAY_ENDPOINT, GATEWAY_MODEL},
    live::{Request, Update, Work},
    plugin_store::Store,
    plugins::{Connection, SettingsFocus},
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use model_access::ApiKey;

fn key(app: &mut App, code: KeyCode) {
    assert!(app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))));
}

fn open_settings(app: &mut App) {
    app.open_plugins();
    for _ in 0..5 {
        if app.plugins.selected_definition().id == "jev" {
            key(app, KeyCode::Enter);
            assert!(app.screen == Screen::PluginSettings);
            return;
        }
        key(app, KeyCode::Down);
    }
    panic!("The bundled Jev plugin was not selectable.");
}

fn replace_field(app: &mut App, focus: SettingsFocus, value: &str) {
    app.plugins.bundled.focus = focus;
    let length = match focus {
        SettingsFocus::Endpoint => app.plugins.bundled.endpoint_field().0.chars().count(),
        SettingsFocus::Model => app.plugins.bundled.field(false).0.chars().count(),
        SettingsFocus::ApiKey => app.plugins.bundled.field(true).0.chars().count(),
        _ => panic!("This helper only edits Jev settings fields."),
    };
    key(app, KeyCode::Home);
    for _ in 0..length {
        key(app, KeyCode::Delete);
    }
    assert!(app.handle(Event::Paste(value.into())));
}

fn test_key(app: &mut App) -> Request {
    app.plugins.bundled.focus = SettingsFocus::TestKey;
    key(app, KeyCode::Enter);
    app.request.take().expect("A Jev key check was not queued.")
}

fn assert_check(request: &Request, endpoint: &str, model: &str, credential: &str) {
    let Work::CheckJev {
        endpoint: actual_endpoint,
        model: actual_model,
    } = &request.kind
    else {
        panic!("The settings form queued something other than a Jev key check.");
    };
    assert_eq!(actual_endpoint, endpoint);
    assert_eq!(actual_model, model);
    assert_eq!(request.key.expose(), credential);
}

#[test]
fn vercel_preset_checks_its_model_and_key_without_saving() {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    open_settings(&mut app);
    app.plugins.bundled.focus = SettingsFocus::Gateway;
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.plugins.bundled.gateway_label(), "Vercel AI Gateway");
    assert_eq!(
        app.plugins.bundled.endpoint_for_check().unwrap(),
        GATEWAY_ENDPOINT
    );
    assert_eq!(
        app.plugins.bundled.model_for_check().unwrap(),
        GATEWAY_MODEL
    );
    replace_field(&mut app, SettingsFocus::ApiKey, "fixture-gateway-key");
    let request = test_key(&mut app);
    assert_check(
        &request,
        GATEWAY_ENDPOINT,
        GATEWAY_MODEL,
        "fixture-gateway-key",
    );
    assert_eq!(app.plugins.bundled.jev_endpoint(), DEFAULT_ENDPOINT);
    assert!(app.plugins.bundled.jev_key().is_none());
    app.apply_update(Update::CheckedJev {
        id: request.id,
        result: Ok(vec![GATEWAY_MODEL.into()]),
    });
    assert!(matches!(
        app.plugins.bundled.connection,
        Connection::Verified
    ));
}

#[test]
fn custom_gateway_check_captures_all_draft_connection_fields() {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    open_settings(&mut app);
    replace_field(
        &mut app,
        SettingsFocus::Endpoint,
        "http://127.0.0.1:12345/fixture/",
    );
    replace_field(&mut app, SettingsFocus::Model, "fixture/decision-model");
    replace_field(&mut app, SettingsFocus::ApiKey, "fixture-custom-key");
    let request = test_key(&mut app);
    assert_check(
        &request,
        "http://127.0.0.1:12345/fixture",
        "fixture/decision-model",
        "fixture-custom-key",
    );
    assert_eq!(app.plugins.bundled.jev_endpoint(), DEFAULT_ENDPOINT);
    assert_eq!(app.plugins.bundled.jev_model(), jev::defaults::MODEL);
    assert!(app.plugins.bundled.jev_key().is_none());
    assert!(app.screen == Screen::PluginSettings);
}

#[test]
fn editing_a_checked_connection_cancels_the_check_and_ignores_its_late_reply() {
    for focus in [
        SettingsFocus::ApiKey,
        SettingsFocus::Endpoint,
        SettingsFocus::Model,
    ] {
        for paste in [false, true] {
            let mut app = App::default();
            app.set_mode(Mode::Live);
            open_settings(&mut app);
            replace_field(&mut app, SettingsFocus::ApiKey, "fixture-check-key");
            let request = test_key(&mut app);
            assert!(app.checking_key && app.checking_jev);
            app.plugins.bundled.focus = focus;
            if paste {
                assert!(app.handle(Event::Paste("x".into())));
            } else {
                key(&mut app, KeyCode::Char('x'));
            }
            assert_ne!(app.request_id, request.id);
            assert!(!app.checking_key && !app.checking_jev);
            assert!(app.request.is_none());
            assert!(matches!(
                app.plugins.bundled.connection,
                Connection::Unchecked
            ));
            app.apply_update(Update::CheckedJev {
                id: request.id,
                result: Ok(vec![jev::defaults::MODEL.into()]),
            });
            assert!(matches!(
                app.plugins.bundled.connection,
                Connection::Unchecked
            ));
        }
    }
    for focus in [SettingsFocus::Gateway, SettingsFocus::RemoveKey] {
        let mut app = App::default();
        app.set_mode(Mode::Live);
        open_settings(&mut app);
        replace_field(&mut app, SettingsFocus::ApiKey, "fixture-check-key");
        let request = test_key(&mut app);
        app.plugins.bundled.focus = focus;
        key(&mut app, KeyCode::Enter);
        assert_ne!(app.request_id, request.id);
        assert!(!app.checking_key && !app.checking_jev);
        assert!(app.plugins.bundled.key_for_check().is_none());
        app.apply_update(Update::CheckedJev {
            id: request.id,
            result: Ok(vec![jev::defaults::MODEL.into()]),
        });
        assert!(matches!(
            app.plugins.bundled.connection,
            Connection::Unchecked
        ));
    }
}

#[test]
fn saved_gateway_settings_restore_and_do_not_reuse_a_typesafe_key() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.load_plugin_settings(Store::under(directory.path()))
        .unwrap();
    app.plugins.bootstrap_credentials(Imported {
        jev_key: Some(ApiKey::new("fixture-typesafe-key")),
        ..Imported::default()
    });
    open_settings(&mut app);
    app.plugins.bundled.focus = SettingsFocus::Gateway;
    key(&mut app, KeyCode::Enter);
    assert!(app.plugins.bundled.key_for_check().is_none());
    app.plugins.bundled.focus = SettingsFocus::Save;
    key(&mut app, KeyCode::Enter);
    assert!(app.screen == Screen::PluginSettings);
    assert!(app.plugins.bundled.error.is_some());
    assert!(app.request.is_none());

    replace_field(&mut app, SettingsFocus::ApiKey, "fixture-vercel-key");
    app.plugins.bundled.focus = SettingsFocus::Save;
    key(&mut app, KeyCode::Enter);
    assert!(app.screen == Screen::Plugins);
    assert_check(
        &app.request.take().unwrap(),
        GATEWAY_ENDPOINT,
        GATEWAY_MODEL,
        "fixture-vercel-key",
    );

    let mut restored = App::default();
    restored.set_mode(Mode::Live);
    restored
        .load_plugin_settings(Store::under(directory.path()))
        .unwrap();
    restored.plugins.bootstrap_credentials(Imported {
        jev_key: Some(ApiKey::new("fixture-typesafe-key")),
        jev_endpoint: Some(DEFAULT_ENDPOINT.into()),
        jev_model: Some("fixture/unrelated-model".into()),
        ..Imported::default()
    });
    assert_eq!(restored.plugins.bundled.jev_endpoint(), GATEWAY_ENDPOINT);
    assert_eq!(restored.plugins.bundled.jev_model(), GATEWAY_MODEL);
    assert_eq!(
        restored.plugins.bundled.jev_key().unwrap().expose(),
        "fixture-vercel-key"
    );

    open_settings(&mut restored);
    restored.plugins.bundled.focus = SettingsFocus::RemoveKey;
    key(&mut restored, KeyCode::Enter);
    restored.plugins.bundled.focus = SettingsFocus::Save;
    key(&mut restored, KeyCode::Enter);
    let mut without_key = App::default();
    without_key.set_mode(Mode::Live);
    without_key
        .load_plugin_settings(Store::under(directory.path()))
        .unwrap();
    without_key.plugins.bootstrap_credentials(Imported {
        jev_key: Some(ApiKey::new("fixture-typesafe-key")),
        ..Imported::default()
    });
    assert_eq!(without_key.plugins.bundled.jev_endpoint(), GATEWAY_ENDPOINT);
    assert_eq!(without_key.plugins.bundled.jev_model(), GATEWAY_MODEL);
    assert!(without_key.plugins.bundled.jev_key().is_none());
}
