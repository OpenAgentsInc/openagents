use oa_desktop_audio::native_transport::{
    start_native_transport, NativeTransportConfig, NativeTransportEvent, NativeTransportHandle,
};
use oa_desktop_audio::{HelperCommand, MediaLifecycle};
use std::io::{self, BufRead, Write};

fn main() {
    // Closed newline-delimited JSON control protocol. Raw media never crosses
    // this channel; production capture/socket/playback backends remain inside
    // this process and die automatically when stdin closes with the parent.
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut lifecycle = MediaLifecycle::default();
    let mut transport: Option<NativeTransportHandle> = None;
    for line in stdin.lock().lines() {
        let decoded = line
            .ok()
            .and_then(|line| serde_json::from_str::<HelperCommand>(&line).ok());
        let state = match decoded {
            Some(command) => {
                if let HelperCommand::Start {
                    identity,
                    gateway_url: Some(gateway_url),
                    application_grant: Some(application_grant),
                    ..
                } = &command
                {
                    if let Some(active) = transport.take() {
                        active.stop();
                    }
                    match start_native_transport(
                        NativeTransportConfig {
                            gateway_url: gateway_url.clone(),
                            application_grant: application_grant.clone(),
                        },
                        identity.clone(),
                    ) {
                        Ok((handle, events)) => {
                            transport = Some(handle);
                            std::thread::spawn(move || {
                                for event in events {
                                    let value = match event {
                                        NativeTransportEvent::Live => {
                                            serde_json::json!({"state":"live"})
                                        }
                                        NativeTransportEvent::Ack {
                                            generation,
                                            sequence,
                                        } => {
                                            serde_json::json!({"state":"ack","generation":generation,"sequence":sequence})
                                        }
                                        NativeTransportEvent::Sent {
                                            generation,
                                            sequence,
                                            payload_length,
                                            sha256,
                                        } => {
                                            serde_json::json!({"state":"packet","generation":generation,"sequence":sequence,"payloadLength":payload_length,"sha256":sha256})
                                        }
                                        NativeTransportEvent::Backpressured => {
                                            serde_json::json!({"state":"backpressured"})
                                        }
                                        NativeTransportEvent::DeviceChanged => {
                                            serde_json::json!({"state":"device_changed"})
                                        }
                                        NativeTransportEvent::Offline => {
                                            serde_json::json!({"state":"offline"})
                                        }
                                        NativeTransportEvent::Revoked => {
                                            serde_json::json!({"state":"revoked"})
                                        }
                                        NativeTransportEvent::Crashed => {
                                            serde_json::json!({"state":"crashed"})
                                        }
                                    };
                                    println!("{}", value);
                                }
                            });
                        }
                        Err(_) => {
                            println!("{}", serde_json::json!({"state":"crashed"}));
                        }
                    }
                }
                if let HelperCommand::SetCapture { enabled } = &command {
                    if let Some(active) = transport.as_ref() {
                        let _ = active.set_capture_enabled(*enabled);
                    }
                }
                if matches!(&command, HelperCommand::Stop { .. }) {
                    if let Some(active) = transport.take() {
                        active.stop();
                    }
                }
                lifecycle.apply(command).clone()
            }
            None => oa_desktop_audio::HelperState::Refused {
                reason: "invalid_control_frame".into(),
            },
        };
        if serde_json::to_writer(&mut stdout, &state).is_err() {
            break;
        }
        if writeln!(&mut stdout).is_err() || stdout.flush().is_err() {
            break;
        }
    }
}
