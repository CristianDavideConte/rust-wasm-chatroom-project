#![recursion_limit = "256"]

use wasm_bindgen::prelude::*;
use wasm_bindgen::*;
use web_sys::{
    console, HtmlInputElement, KeyboardEvent, MessageEvent
};
use yew::{html, App, Component, ComponentLink, Html, NodeRef, ShouldRender};

pub struct ChatComponent {
    link: ComponentLink<Self>,
    chat: NodeRef,
    input_field: NodeRef,
    websocket: web_sys::WebSocket,
}

pub enum UserEvent {
    Ignore,
    SendMessage
}

fn escape(message: &str) -> String {
    message
    .replace("&", "&amp;")
    .replace("<", "&lt;")
    .replace(">", "&gt;")
    .replace("\"", "&quot;")
    .replace("\'", "&#039;")
} 

fn update_chat(message: String) {
    //Update the on-screen chat
    let document: web_sys::Document = web_sys::window()
        .expect("window not available")
        .document()
        .unwrap();
    let chat = document.get_element_by_id("chat").unwrap();
    let mut history = chat.inner_html();
    history.push_str("<div class = \"message-container\">");
    history.push_str(&message);
    history.push_str("</div>");
    chat.set_inner_html(&history);
}

impl Component for ChatComponent {
    type Message = UserEvent;
    type Properties = ();

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        let url = "ws://localhost:8081";
        let websocket = web_sys::WebSocket::new(url).unwrap();
        websocket.set_binary_type(web_sys::BinaryType::Arraybuffer);
        let cloned_ws = websocket.clone();

        let onopen_callback = Closure::wrap(Box::new(move |_| { //Get the chat history
            let msg = "get::".to_owned(); //Tell the server we are sending a new message
            match cloned_ws.send_with_str(&msg) {
                Ok(()) => {}
                Err(err) => {
                    console::log_1(&JsValue::from(format!(
                        "Cannot retrieve chat history: {:?}",
                        err
                    )));
                }
            }
        }) as Box<dyn FnMut(JsValue)>);

        let onmessage_callback = Closure::wrap(Box::new(move |event: MessageEvent| {
            if let Ok(new_message) = event.data().dyn_into::<js_sys::JsString>() {
                match new_message.as_string() {
                    Some(message) => {
                        update_chat(message);
                    }
                    _ => {}
                }
            } //Ignore the error
        }) as Box<dyn FnMut(MessageEvent)>);

        websocket.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        websocket.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));

        //Forget the callback to keep it alive
        onopen_callback.forget();
        onmessage_callback.forget();

        ChatComponent {
            link,
            chat: NodeRef::default(),
            input_field: NodeRef::default(),
            websocket: websocket,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            UserEvent::SendMessage => {
                let input = self.input_field.cast::<HtmlInputElement>().unwrap();
                let input_value = input.value();
                let message = input_value.trim();

                if message != "" {
                    //Avoid sending empty messages
                    let mut msg = "post::".to_owned(); //Tell the server we are sending a new message
                    msg.push_str(&escape(message));
                    match self.websocket.send_with_str(&msg) {
                        Ok(()) => {
                            input.set_value("");
                            input.focus().unwrap();
                        }
                        Err(err) => {
                            console::log_1(&JsValue::from(format!("message error {:?}", err)));
                        }
                    }
                }
                return true;
            }
            UserEvent::Ignore => {
                return false;
            }
        }
    }

    fn change(&mut self, _: Self::Properties) -> ShouldRender {
        false
    }

    fn view(&self) -> Html {
        let send_callback = self
            .link
            .callback(|_: web_sys::MouseEvent| UserEvent::SendMessage);
        let send_callback_on_enter = self.link.callback(|key: KeyboardEvent| {
            if key.key() == "Enter" {
                UserEvent::SendMessage
            } else {
                UserEvent::Ignore
            }
        });

        html! {
            <div id = "container">
                <h1 id = "title">{"Chat Room"}</h1>
                <div id = "chat" ref=self.chat.clone()></div>
                <div id = "message-inputs">
                    <input ref=self.input_field.clone() placeholder = "Type a message" onkeyup=send_callback_on_enter/>
                    <button class = "fas fa-paper-plane" onclick=send_callback></button>
                </div>
            </div>
        }
    }
}

#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once(); //Serve per rendere leggibili gli errori di WASM
}

#[wasm_bindgen(start)]
pub fn run_app() {
    init_panic_hook();
    App::<ChatComponent>::new().mount_to_body();
}
