pub mod ws;

use leptos::{Show, create_signal, create_rw_signal, create_effect, component, mount_to_body, RwSignal, IntoView, SignalUpdate, SignalGet, view};
use ws::{WebSocketStatus, WebSocketMessage, WebSocketTask, WsAction, WebSocketService};


const WS_ECHO_SERVER: &str = "wss://ws.postman-echo.com/raw";

#[component]
pub fn WebSocketEcho() -> impl IntoView {
    let ws: RwSignal<Option<WebSocketTask>> = create_rw_signal(None);
    let (status, set_status) = create_signal(WebSocketStatus::Connecting);
    let (data, set_data) = create_signal(WebSocketMessage::Text("".to_string()));

    create_effect(move |_| {
        ws.update(move |x| {
            *x = WebSocketService::connect(
                WS_ECHO_SERVER,
                WsAction::Leptos(set_data),
                WsAction::Leptos(set_status),
            )
            .ok()
        });
    });

    create_effect(move |_| match status.get() {
        WebSocketStatus::Opened => {
            log::debug!("Video WebSocket opened");
        }
        WebSocketStatus::Closed => {
            log::debug!("Video WebSocket closed");
            ws.update(move |x| *x = None);
        }
        WebSocketStatus::Error(e) => {
            log::debug!("Video WebSocket error: {:?}", e);
            ws.update(move |x| *x = None);
        }
        WebSocketStatus::Connecting => {
            log::debug!("Video WebSocket connecting");
        }
    });

    create_effect(move |_| {
        if let WebSocketMessage::Text(text) = data.get() {
            log::debug!("WebSocket data: {}", text);
        } else if let WebSocketMessage::Binary(data) = data.get() {
            log::debug!("WebSocket data: {:?}", data);
        }
    });

    let echo_random_string = move |_| {
        if let Some(ws) = ws.get() {
            ws.send(format!("Random string: {}", rand::random::<u64>()));
        }
    };

    view! {
        <div>
            <button on:click=echo_random_string>Echo Random String</button>
            <Show when=move || !data.get().is_empty() fallback=move || view!{{}}>
                <p>{move || data.get().to_string()}</p>
            </Show>
            
        </div>
    }
}

// Easy to use with Trunk (trunkrs.dev) or with a simple wasm-bindgen setup
pub fn main() {
    mount_to_body(|| view! {
        <WebSocketEcho />
    })
}