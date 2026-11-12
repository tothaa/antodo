//! MCP page presentation; transport, protocol and task commands live in core.
use super::app::{App, Message};
use crate::core::{self, mcp::Config};
use iced::widget::{button, checkbox, column, container, row, scrollable, text, text_input};
use iced::{Element, Fill};

#[derive(Debug, Clone, Copy)]
pub enum Field {
    Address,
    Port,
    Token,
}
pub struct Settings {
    pub address: String,
    pub port: String,
    pub token: String,
    pub allow_edits: bool,
}
impl Default for Settings {
    fn default() -> Self {
        let c = Config::default();
        Self {
            address: c.address.to_string(),
            port: c.port.to_string(),
            token: c.token,
            allow_edits: c.allow_edits,
        }
    }
}
impl Settings {
    pub fn config(&self) -> core::Result<Config> {
        let config = Config {
            address: self
                .address
                .parse()
                .map_err(|_| core::invalid("Enter a loopback IP address such as 127.0.0.1"))?,
            port: self.port.parse().map_err(|_| {
                core::invalid("Port must be a number from 0 to 65535 (0 chooses a free port)")
            })?,
            token: self.token.clone(),
            allow_edits: self.allow_edits,
        };
        config.validate()?;
        Ok(config)
    }
}
pub fn snippet(url: &str) -> String {
    format!("[mcp_servers.antodo]\nurl = {url:?}\nbearer_token_env_var = \"ANTODO_MCP_TOKEN\"\n")
}
pub fn view(app: &App) -> Element<'_, Message> {
    let running = app.mcp_server.is_some();
    let settings = &app.mcp_settings;
    let field = |label: &'static str, value, which, secret| {
        column![
            text(label).size(13),
            text_input(label, value)
                .secure(secret)
                .on_input_maybe((!running).then_some(move |value| Message::McpField(which, value)))
                .padding(10)
        ]
        .spacing(5)
    };
    let url = app.mcp_server.as_ref().map(|s| s.url()).unwrap_or_else(|| {
        settings
            .config()
            .map(|c| {
                format!(
                    "http://{}/mcp",
                    std::net::SocketAddr::new(c.address, c.port)
                )
            })
            .unwrap_or_else(|_| "Configure a valid loopback address and port".into())
    });
    let form = column![
        text(if running { "Server running" } else { "Server stopped" }).size(23),
        text(format!("Workspace: {}", app.filename())),
        row![field("Bind address", &settings.address, Field::Address, false), field("Port", &settings.port, Field::Port, false)].spacing(12),
        row![field("Access token", &settings.token, Field::Token, true).width(Fill), button("Copy token").on_press(Message::McpCopyToken)].spacing(12).align_y(iced::Alignment::End),
        checkbox(settings.allow_edits).label("Allow clients to create, update and delete tasks").on_toggle_maybe((!running).then_some(Message::McpEdits)),
        row![button("Start").on_press_maybe((!running).then_some(Message::McpStart)), button("Stop").style(button::danger).on_press_maybe(running.then_some(Message::McpStop))].spacing(12),
        text(url.clone()).font(iced::Font::MONOSPACE),
        text("Start shares this working document. Closing or replacing the workspace stops the server. Read-only is the default; external edits stay unsaved until you choose Save. Close task drawers and finish local edits before allowing external changes.").size(14),
    ].spacing(14);
    let instructions = column![
        text("Connect a local MCP client").size(23),
        text("1. Start the server. Copy the token into the client's bearer-token setting, or set ANTODO_MCP_TOKEN in the environment that launches Codex."),
        text("2. For Codex, add this to its config.toml, then restart/reconnect the client. For other clients, select Streamable HTTP, use the URL below, and send Authorization: Bearer <access token>."),
        container(text(snippet(&url)).font(iced::Font::MONOSPACE)).padding(12).style(container::rounded_box),
        button("Copy Codex config").on_press(Message::McpCopyConfig),
        text("3. Ask the client to list antodo tasks. Available tools: list_tasks, get_task, list_tags. Enabling edits also exposes create_task, update_task and delete_task. Lists paginate with offset/limit; dates and task IDs are returned by the tools."),
        button("Codex connection documentation").style(button::text).on_press(Message::McpDocs(false)),
        text("ChatGPT and cloud-hosted clients").size(23),
        text("A cloud client cannot reach this machine's localhost directly. Use a supported private MCP tunnel or an authenticated gateway that can reach this URL and supply the bearer token. For ChatGPT, follow the official Secure MCP Tunnel setup, configure upstream authentication, then add the tunnel as a custom MCP server. Availability depends on your workspace and permissions. Antodo does not create a public endpoint or implement OAuth itself."),
        button("ChatGPT / Secure MCP Tunnel instructions").style(button::text).on_press(Message::McpDocs(true)),
    ].spacing(14);
    scrollable(
        column![
            container(form).padding(20).style(container::rounded_box),
            container(instructions)
                .padding(20)
                .style(container::rounded_box)
        ]
        .spacing(20),
    )
    .height(Fill)
    .into()
}

#[cfg(test)]
mod tests {
    use super::super::views::render_tests::{Screen, render};
    use super::*;

    #[test]
    fn mcp_form_hides_token_and_disables_start_while_running() {
        let mut app = super::super::preview::test_app();
        app.mcp_settings.port = "0".into();
        let snapshot = render(view(&app), &app.theme(), 1000.0);
        assert!(snapshot.contains("Server stopped"));
        assert!(!snapshot.inputs.contains(&app.mcp_settings.token));
        app.mcp_settings.address = "invalid".into();
        assert!(
            render(view(&app), &app.theme(), 1000.0)
                .contains("Configure a valid loopback address and port")
        );
        app.mcp_settings.address = "127.0.0.1".into();
        app.mcp_server =
            Some(core::mcp::Server::start(app.mcp_settings.config().unwrap()).unwrap());
        let mut screen = Screen::new(view(&app), iced::Size::new(1000.0, 900.0));
        screen.draw(&app.theme());
        assert!(screen.snapshot().contains("Server running"));
        assert!(screen.click("Start").is_empty());
        assert!(matches!(
            screen.click("Stop").as_slice(),
            [Message::McpStop]
        ));
    }
}
