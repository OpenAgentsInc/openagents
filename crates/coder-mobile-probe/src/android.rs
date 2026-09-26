//! Native Android text controls created from Rust on the UI thread.

use android_activity::{AndroidApp, MainEvent, OnCreateState, PollEvent};
use jni::{JavaVM, jni_sig, jni_str, objects::JObject, objects::JValue};

#[unsafe(no_mangle)]
fn android_on_create(state: &OnCreateState<'_>) {
    // SAFETY: Android owns this VM, and the activity reference stays valid for
    // the callback. No borrowed reference escapes the attached UI thread.
    let vm = unsafe { JavaVM::from_raw(state.vm_as_ptr().cast()) };
    let result = vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        let raw = state.activity_as_ptr() as jni::sys::jobject;
        let activity = unsafe { env.as_cast_raw::<JObject>(&raw)? };
        let window = env
            .call_method(
                &activity,
                jni_str!("getWindow"),
                jni_sig!("()Landroid/view/Window;"),
                &[],
            )?
            .l()?;
        let null = JObject::null();
        // NativeActivity normally gives its surface and input queue to native
        // drawing code. This probe uses Android widgets, which need the normal
        // View rendering and input dispatch paths instead.
        env.call_method(
            &window,
            jni_str!("takeSurface"),
            jni_sig!("(Landroid/view/SurfaceHolder$Callback2;)V"),
            &[JValue::Object(&null)],
        )?;
        env.call_method(
            &window,
            jni_str!("takeInputQueue"),
            jni_sig!("(Landroid/view/InputQueue$Callback;)V"),
            &[JValue::Object(&null)],
        )?;
        let column = env.new_object(
            jni_str!("android/widget/LinearLayout"),
            jni_sig!("(Landroid/content/Context;)V"),
            &[JValue::Object(&activity)],
        )?;
        env.call_method(
            &column,
            jni_str!("setOrientation"),
            jni_sig!("(I)V"),
            &[JValue::Int(1)],
        )?;
        env.call_method(
            &column,
            jni_str!("setFitsSystemWindows"),
            jni_sig!("(Z)V"),
            &[JValue::Bool(true)],
        )?;
        let input = env.new_object(
            jni_str!("android/widget/EditText"),
            jni_sig!("(Landroid/content/Context;)V"),
            &[JValue::Object(&activity)],
        )?;
        let hint = env.new_string("Input and IME probe. This field sends nothing.")?;
        env.call_method(
            &input,
            jni_str!("setHint"),
            jni_sig!("(Ljava/lang/CharSequence;)V"),
            &[JValue::Object(&hint)],
        )?;
        env.call_method(
            &column,
            jni_str!("addView"),
            jni_sig!("(Landroid/view/View;)V"),
            &[JValue::Object(&input)],
        )?;
        let rows = env.new_object(jni_str!("java/util/ArrayList"), jni_sig!("()V"), &[])?;
        let mut messages = vec!["Coder platform probe. Synthetic, read-only task. Cost: unknown. Checks: not run. No network, credentials, execution, or control.".to_owned()];
        messages.extend(crate::fixture().iter().map(|step| step.message.clone()));
        messages.push("END OF COMPLETE SYNTHETIC TRANSCRIPT".to_owned());
        for message in messages {
            let text = env.new_string(message)?;
            env.call_method(&rows, jni_str!("add"), jni_sig!("(Ljava/lang/Object;)Z"), &[JValue::Object(&text)])?;
            env.delete_local_ref(text);
        }
        let layout = env.get_static_field(jni_str!("android/R$layout"), jni_str!("simple_list_item_1"), jni_sig!("I"))?.i()?;
        let adapter = env.new_object(jni_str!("android/widget/ArrayAdapter"), jni_sig!("(Landroid/content/Context;ILjava/util/List;)V"), &[JValue::Object(&activity), JValue::Int(layout), JValue::Object(&rows)])?;
        let list = env.new_object(jni_str!("android/widget/ListView"), jni_sig!("(Landroid/content/Context;)V"), &[JValue::Object(&activity)])?;
        env.call_method(&list, jni_str!("setAdapter"), jni_sig!("(Landroid/widget/ListAdapter;)V"), &[JValue::Object(&adapter)])?;
        let intent = env.call_method(&activity, jni_str!("getIntent"), jni_sig!("()Landroid/content/Intent;"), &[])?.l()?;
        let start_name = env.new_string("start_row")?;
        let start = env.call_method(&intent, jni_str!("getIntExtra"), jni_sig!("(Ljava/lang/String;I)I"), &[JValue::Object(&start_name), JValue::Int(0)])?.i()?;
        env.call_method(&list, jni_str!("setSelection"), jni_sig!("(I)V"), &[JValue::Int(start.clamp(0, crate::STEP_COUNT as i32 + 1))])?;
        let layout = env.new_object(jni_str!("android/widget/LinearLayout$LayoutParams"), jni_sig!("(IIF)V"), &[JValue::Int(-1), JValue::Int(0), JValue::Float(1.0)])?;
        env.call_method(&column, jni_str!("addView"), jni_sig!("(Landroid/view/View;Landroid/view/ViewGroup$LayoutParams;)V"), &[JValue::Object(&list), JValue::Object(&layout)])?;
        env.call_method(
            &activity,
            jni_str!("setContentView"),
            jni_sig!("(Landroid/view/View;)V"),
            &[JValue::Object(&column)],
        )?;
        match crate::android_keystore::probe(env) {
            Ok(receipt) => eprintln!("coder-mobile-probe: {receipt}"),
            Err(error) => eprintln!("coder-mobile-probe: keystore failed: {error}"),
        }
        Ok(())
    });
    if let Err(error) = result {
        eprintln!("coder-mobile-probe: native view failed: {error}");
    }
}

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    let mut destroyed = false;
    while !destroyed {
        app.poll_events(None, |event| {
            if let PollEvent::Main(main) = event {
                let name = match main {
                    MainEvent::Start => "start",
                    MainEvent::Resume { .. } => "resume",
                    MainEvent::Pause => "pause",
                    MainEvent::Stop => "stop",
                    MainEvent::Destroy => "destroy",
                    _ => "window_or_input_event",
                };
                eprintln!("coder-mobile-probe: lifecycle {name}");
                destroyed = matches!(main, MainEvent::Destroy);
            }
        });
    }
}
