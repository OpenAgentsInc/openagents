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
<doc xmlns="https://hyperview.org/hyperview">
  <screen>
    <styles>
      <style id="main" height="800" backgroundColor="black" />
      <style id="container" flex="1" alignItems="center" justifyContent="flex-end" paddingBottom="24" />
      <style id="header" width="100%" marginLeft="24" marginBottom="16" />
      <style id="headerText" color="#808080" fontSize="16" />
      <style id="inputContainer" width="100%" flexDirection="row" alignItems="center" marginLeft="24" marginRight="24" borderWidth="1" borderColor="#808080" borderRadius="8" paddingLeft="12" paddingRight="12" paddingTop="8" paddingBottom="8" />
      <style id="input" flex="1" color="white" fontSize="16" />
      <style id="submitButton" width="24" height="24" justifyContent="center" alignItems="center" marginLeft="8" />
      <style id="submitArrow" color="#808080" fontSize="24" />
    </styles>
    <body>
      <view style="main">
        <view style="container">
          <view style="header">
            <text style="headerText">New conversation</text>
            <text style="headerText">Would you like to learn about what I can do?</text>
          </view>
          <view style="inputContainer">
            <text-field 
              style="input"
              placeholder="Ask anything..."
              placeholderTextColor="#808080"
            />
            <view style="submitButton">
              <text style="submitArrow">↑</text>
            </view>
          </view>
        </view>
      </view>
    </body>
  </screen>
</doc>"###
            .into(),
        )
        .unwrap()
}
