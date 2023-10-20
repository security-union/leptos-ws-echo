//! A service to connect to a server through the
//! [`WebSocket` Protocol](https://tools.ietf.org/html/rfc6455).

/**
* Copyright (c) 2017 Denis Kolodin

Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
*/
use std::{
    fmt,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use gloo::events::EventListener;
use js_sys::Uint8Array;
use leptos::{view, IntoView, SignalUpdate, WriteSignal};
use log::error;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{BinaryType, Event, MessageEvent, WebSocket};

/// The status of a WebSocket connection. Used for status notifications.
#[derive(Clone, Debug, PartialEq)]
pub enum WebSocketStatus {
    /// Fired when a WebSocket connection has opened.
    Opened,
    /// Fired when a WebSocket connection has closed.
    Closed,
    /// Fired when a WebSocket connection has failed.
    Error(JsValue),
    /// Fired when a WebSocket connection is connecting.
    Connecting,
}

impl IntoView for WebSocketStatus {
    fn into_view(self) -> leptos::View {
        match self {
            self::WebSocketStatus::Opened => view! {
                <p>"WebSocket Status: Opened"</p>
            },
            self::WebSocketStatus::Closed => view! {
                <p>"WebSocket Status: Closed"</p>
            },
            self::WebSocketStatus::Error(_e) => view! {
                <p>"WebSocket Status: Error"</p>
            },
            self::WebSocketStatus::Connecting => view! {
                <p>"WebSocket Status: Connecting"</p>
            },
        }
        .into_view()
    }
}

#[derive(Clone, Debug)]
pub enum WebSocketMessage {
    Text(String),
    Binary(Vec<u8>),
}

impl WebSocketMessage {
    pub fn is_empty(&self) -> bool {
        match self {
            WebSocketMessage::Text(text) => text.is_empty(),
            WebSocketMessage::Binary(data) => data.is_empty(),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            WebSocketMessage::Text(text) => text.clone(),
            WebSocketMessage::Binary(data) => {
                let mut out = String::new();
                for byte in data {
                    out.push_str(&format!("{:02x}", byte));
                }
                out
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// An error encountered by a WebSocket.
pub enum WebSocketError {
    #[error("{0}")]
    /// An error encountered when creating the WebSocket.
    CreationError(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum WsAction<T>
where
    T: 'static,
{
    Leptos(WriteSignal<T>),
}

/// A handle to control the WebSocket connection. Implements `Task` and could be canceled.
#[must_use = "the connection will be closed when the task is dropped"]
#[derive(Clone)]
pub struct WebSocketTask {
    ws: WebSocket,
    notification: WsAction<WebSocketStatus>,
    #[allow(dead_code)]
    listeners: [Rc<EventListener>; 4],
}

impl WebSocketTask {
    fn new(
        ws: WebSocket,
        notification: WsAction<WebSocketStatus>,
        listener_0: EventListener,
        listeners: [EventListener; 3],
    ) -> WebSocketTask {
        let [listener_1, listener_2, listener_3] = listeners;
        WebSocketTask {
            ws,
            notification,
            listeners: [
                Rc::new(listener_0),
                Rc::new(listener_1),
                Rc::new(listener_2),
                Rc::new(listener_3),
            ],
        }
    }
}

impl fmt::Debug for WebSocketTask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WebSocketTask")
    }
}

impl Deref for WebSocketTask {
    type Target = WebSocket;

    fn deref(&self) -> &WebSocket {
        &self.ws
    }
}

impl DerefMut for WebSocketTask {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ws
    }
}

/// A WebSocket service attached to a user context.
#[derive(Default, Debug)]
pub struct WebSocketService {}

impl WebSocketService {
    /// Connects to a server through a WebSocket connection. Needs two callbacks; one is passed
    /// data, the other is passed updates about the WebSocket's status.
    pub fn connect(
        url: &str,
        callback: WsAction<WebSocketMessage>,
        notification: WsAction<WebSocketStatus>,
    ) -> Result<WebSocketTask, WebSocketError> {
        update_ws_action(&notification, WebSocketStatus::Connecting);
        let ConnectCommon(ws, listeners) = Self::connect_common(url, notification.clone())?;
        let listener = EventListener::new(&ws, "message", move |event: &Event| {
            let event = event.dyn_ref::<MessageEvent>().unwrap();
            process_both(event, &callback);
        });
        Ok(WebSocketTask::new(ws, notification, listener, listeners))
    }

    fn connect_common(
        url: &str,
        notification: WsAction<WebSocketStatus>,
    ) -> Result<ConnectCommon, WebSocketError> {
        let ws = WebSocket::new(url);

        let ws = ws.map_err(|ws_error| {
            WebSocketError::CreationError(
                ws_error
                    .unchecked_into::<js_sys::Error>()
                    .to_string()
                    .as_string()
                    .unwrap(),
            )
        })?;

        ws.set_binary_type(BinaryType::Arraybuffer);
        let notify = notification.clone();
        let listener_open = move |_: &Event| {
            update_ws_action(&notify, WebSocketStatus::Opened);
        };
        let notify = notification.clone();
        let listener_close = move |_: &Event| {
            update_ws_action(&notify, WebSocketStatus::Closed);
        };
        let notify = notification.clone();
        let listener_error = move |e: &Event| {
            let error = format!("{:?}", e);
            update_ws_action(&notify, WebSocketStatus::Error(JsValue::from_str(&error)));
        };
        {
            let listeners = [
                EventListener::new(&ws, "open", listener_open),
                EventListener::new(&ws, "close", listener_close),
                EventListener::new(&ws, "error", listener_error),
            ];
            Ok(ConnectCommon(ws, listeners))
        }
    }
}

fn update_ws_action<T: 'static>(action: &WsAction<T>, update: T) {
    match action {
        WsAction::Leptos(ref signal) => {
            signal.update(move |x| *x = update);
        }
    }
}

struct ConnectCommon(WebSocket, [EventListener; 3]);

fn process_binary(event: &MessageEvent, callback: &WsAction<WebSocketMessage>) {
    let bytes = if !event.data().is_string() {
        Some(event.data())
    } else {
        None
    };

    let data = if let Some(bytes) = bytes {
        let bytes: Vec<u8> = Uint8Array::new(&bytes).to_vec();
        Some(bytes)
    } else {
        None
    };

    if let Some(data) = data {
        let out = WebSocketMessage::Binary(data);
        update_ws_action(callback, out);
    } else {
        error!("Received binary data, but couldn't convert it to bytes");
    }
}

fn process_text(event: &MessageEvent, callback: &WsAction<WebSocketMessage>) {
    let text = event.data().as_string();
    if let Some(text) = text {
        update_ws_action(callback, WebSocketMessage::Text(text));
    } else {
        error!("Received text data, but couldn't convert it to a string");
    }
}

fn process_both(event: &MessageEvent, callback: &WsAction<WebSocketMessage>) {
    let is_text = event.data().is_string();
    if is_text {
        process_text(event, callback);
    } else {
        process_binary(event, callback);
    }
}

impl WebSocketTask {
    /// Sends data to a WebSocket connection.
    pub fn send(&self, data: String) {
        let result = self.ws.send_with_str(&data);

        if result.is_err() {
            update_ws_action(
                &self.notification,
                WebSocketStatus::Error(result.err().unwrap()),
            );
        }
    }

    /// Sends binary data to a WebSocket connection.
    pub fn send_binary(&self, data: Vec<u8>) {
        let result = self.ws.send_with_u8_array(&data);

        if result.is_err() {
            log::error!("Send failed");
            update_ws_action(
                &self.notification,
                WebSocketStatus::Error(result.err().unwrap()),
            );
        }
    }

    #[allow(dead_code)]
    fn close(&self) -> Result<(), JsValue> {
        self.ws.close()
    }
}
