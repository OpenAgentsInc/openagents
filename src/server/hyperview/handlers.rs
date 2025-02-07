use crate::server::config::AppState;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::Response,
};

pub async fn hello_world(State(_state): State<AppState>) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(
            r###"<?xml version="1.0" encoding="UTF-8"?>
<doc xmlns="https://hyperview.org/hyperview"
     xmlns:ws="https://openagents.com/hyperview-websocket">
  <screen>
    <styles>
      <style id="main" height="800" backgroundColor="black" />
      <style id="container" flex="1" alignItems="center" justifyContent="flex-end" paddingBottom="24" />
      <style id="header" width="100%" marginLeft="24" marginBottom="16" />
      <style id="headerText" color="#808080" fontSize="16" />
      <style id="messagesContainer" flex="1" width="100%" marginLeft="24" marginRight="24" />
      <style id="message" marginBottom="12" />
      <style id="messageText" color="white" fontSize="16" />
      <style id="inputContainer" width="100%" flexDirection="row" alignItems="center" marginLeft="24" marginRight="24" borderWidth="1" borderColor="#808080" borderRadius="8" paddingLeft="12" paddingRight="12" paddingTop="8" paddingBottom="8" />
      <style id="input" flex="1" color="white" fontSize="16" />
      <style id="submitButton" width="24" height="24" justifyContent="center" alignItems="center" marginLeft="8" />
      <style id="submitArrow" color="#808080" fontSize="24" />
      <style id="statusContainer" position="absolute" top="0" left="0" right="0" height="24" justifyContent="center" alignItems="center" />
      <style id="statusText" fontSize="12" />
      <style id="statusConnected" color="#4CAF50" />
      <style id="statusDisconnected" color="#F44336" />
    </styles>
    <body>
      <!-- WebSocket connection -->
      <behavior
        trigger="load"
        action="ws:connect"
        ws:url="ws://localhost:8000/hyperview/ws"
      />

      <!-- Connection status indicator -->
      <view id="status" style="statusContainer">
        <behavior
          trigger="ws:open"
          action="replace"
          href="/hyperview/fragments/connected-status"
        />
        <behavior
          trigger="ws:close"
          action="replace"
          href="/hyperview/fragments/disconnected-status"
        />
        <text style="statusText,statusDisconnected">Connecting...</text>
      </view>

      <view style="main">
        <view style="container">
          <!-- Messages container -->
          <view id="messages" style="messagesContainer" ws:swap="append">
            <view style="message">
              <text style="messageText">Welcome! Ask me anything.</text>
            </view>
          </view>

          <!-- Input form -->
          <form id="chat-form">
            <view style="inputContainer">
              <text-field 
                name="message"
                style="input"
                placeholder="Ask anything..."
                placeholderTextColor="#808080"
              />
              <view style="submitButton">
                <behavior 
                  trigger="press"
                  action="ws:send"
                  ws:message="$chat-form"
                >
                  <text style="submitArrow">↑</text>
                </behavior>
              </view>
            </view>
          </form>
        </view>
      </view>
    </body>
  </screen>
</doc>"###
            .into(),
        )
        .unwrap()
}

pub async fn main_screen(State(_state): State<AppState>) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(
            r###"<?xml version="1.0" encoding="UTF-8"?>
<doc xmlns="https://hyperview.org/hyperview">
  <screen>
    <styles>
      <style id="container" flex="1" backgroundColor="black" alignItems="center" justifyContent="center" />
      <style id="title" fontSize="24" color="white" marginBottom="32" />
      <style id="button" backgroundColor="white" padding="16" borderRadius="8" marginTop="16" width="240" alignItems="center" />
      <style id="buttonText" color="black" fontSize="16" fontWeight="600" />
    </styles>
    
    <body style="container">
      <text style="title">Welcome to OpenAgents</text>
      
      <!-- Chat Button -->
      <view style="button">
        <behavior 
          trigger="press" 
          action="push"
          href="/hyperview"
        />
        <text style="buttonText">Start Chat</text>
      </view>
    </body>
  </screen>
</doc>"###
            .into(),
        )
        .unwrap()
}

pub async fn connected_status() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(
            r###"<?xml version="1.0" encoding="UTF-8"?>
<text xmlns="https://hyperview.org/hyperview" style="statusText,statusConnected">
  Connected
</text>"###
                .into(),
        )
        .unwrap()
}

pub async fn disconnected_status() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(
            r###"<?xml version="1.0" encoding="UTF-8"?>
<text xmlns="https://hyperview.org/hyperview" style="statusText,statusDisconnected">
  Disconnected - Reconnecting...
</text>"###
                .into(),
        )
        .unwrap()
}