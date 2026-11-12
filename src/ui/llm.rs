use super::app::{App, Message};
use crate::core::llm::Provider;
use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Element, Fill};
#[derive(Debug, Clone, Copy)]
pub enum Field {
    Endpoint,
    Model,
    Key,
}
pub fn page(app: &App) -> Element<'_, Message> {
    let c = &app.llm_settings;
    let field = |label: &'static str, value, which, secure| {
        column![
            text(label).size(13),
            text_input(label, value)
                .secure(secure)
                .on_input(move |v| Message::LlmField(which, v))
                .padding(10)
        ]
        .spacing(5)
    };
    scrollable(column![
 text(if app.llm_config.is_some(){"Chat configured for this session"}else{"Configure an LLM connection"}).size(24),
 pick_list(Provider::ALL,Some(c.provider),Message::LlmProvider),
 field("Chat completions URL (HTTP providers)",&c.endpoint,Field::Endpoint,false),
 field("Model (blank uses Codex's default)",&c.model,Field::Model,false),
 field("API key (not needed for local Ollama or logged-in Codex)",&c.key,Field::Key,true),
 checkbox(c.edits).label("Allow chat to create, update and delete tasks and notes").on_toggle(Message::LlmEdits),
 row![button("Use this connection").on_press(Message::LlmConfigure),button("Disconnect").on_press(Message::LlmDisconnect)].spacing(12),
 text("Settings, API keys and the last six completed chat exchanges are held in memory for this app session. Switching workspaces clears chat history. Selected provider receives your questions and any task or note data its tools read. Changes stay unsaved until Save. Applying changed connection settings clears chat history."),
 text("Setup instructions").size(24),
 text("Ollama: install Ollama, run ollama serve, then pull a tool-capable model (for example: ollama pull qwen3:8b). Use http://127.0.0.1:11434/v1/chat/completions and the exact installed model name. HTTP providers require curl on PATH."),
 text("OpenAI API: create an API key in the OpenAI developer platform, enter a supported chat/tool model and https://api.openai.com/v1/chat/completions. API billing is separate from a ChatGPT subscription."),
 text("Gemini API: create a Gemini API key in Google AI Studio. Use https://generativelanguage.googleapis.com/v1beta/openai/chat/completions and a tool-capable Gemini model available to your account."),
 text("Codex CLI: install the codex command and run codex login in a terminal as this user. Choose Codex CLI here. Antodo launches an ephemeral read-only codex exec process in an empty temporary directory, with user configuration/rules ignored. The CLI must support these flags. It returns proposed task and note actions for antodo to execute. Existing Desktop chats are not attached or imported."),
 text("ChatGPT Desktop: there is no assumed local chat-completions endpoint. Use the MCP page to connect an external compatible client, or choose OpenAI API / Codex CLI for embedded chat here."),
 text("Use Start chat on the workspace or in a task or note editor. Editor chat receives the selected task or note context and reads applied database state, including tags and linked notes/tasks. Apply local draft changes first if you want the assistant to edit; invalid actions are rejected by the same Rust API used by the application."),
 ].spacing(14).padding(20)).height(Fill).into()
}
pub fn dialog(app: &App) -> Element<'_, Message> {
    let mut messages = column![].spacing(12);
    for (role, value) in &app.chat.log {
        messages = messages.push(column![text(role).size(13), text(value)].spacing(4));
    }
    if let Some(error) = &app.chat.error {
        messages = messages.push(text(error).style(text::danger));
    }
    column![
        row![
            text(match app.chat_context {
                Some(crate::core::llm::Context::Task(_)) => "Task chat",
                Some(crate::core::llm::Context::Note(_)) => "Note chat",
                None => "Workspace chat",
            })
            .size(24)
            .width(Fill),
            button("× Close").on_press(Message::ChatClose)
        ]
        .spacing(12),
        text("Reads applied tasks and notes. Save keeps successful changes on disk.").size(13),
        container(scrollable(messages).id("llm-chat-history").height(340))
            .padding(10)
            .style(container::rounded_box),
        text_input(
            "Ask about tasks, notes, or request an action…",
            &app.chat_input
        )
        .on_input(Message::ChatInput)
        .on_submit(Message::ChatSend)
        .padding(12),
        row![
            button(if app.chat.busy() {
                "Thinking…"
            } else {
                "Send"
            })
            .on_press_maybe((!app.chat.busy()).then_some(Message::ChatSend)),
            button("Cancel request").on_press_maybe(app.chat.busy().then_some(Message::ChatCancel)),
            button("Clear conversation").on_press(Message::ChatClear)
        ]
        .spacing(10)
    ]
    .spacing(12)
    .into()
}

#[cfg(test)]
mod tests {
    use super::super::views::render_tests::{Screen, render};
    use super::*;

    #[test]
    fn connection_form_masks_keys_and_chat_controls_emit_expected_messages() {
        let mut app = super::super::preview::test_app();
        app.llm_settings.key = "private-session-key".into();
        let snapshot = render(page(&app), &app.theme(), 1000.0);
        assert!(snapshot.contains("Configure an LLM connection"));
        assert!(!snapshot.inputs.contains(&app.llm_settings.key));
        app.llm_config = Some(app.llm_settings.clone());
        assert!(
            render(page(&app), &app.theme(), 1000.0).contains("Chat configured for this session")
        );
        let mut screen = Screen::new(dialog(&app), iced::Size::new(850.0, 900.0));
        screen.draw(&app.theme());
        assert!(screen.snapshot().contains("Workspace chat"));
        assert!(screen.click("Cancel request").is_empty());
        assert!(matches!(
            screen.click("Send").as_slice(),
            [Message::ChatSend]
        ));
        assert!(matches!(
            screen.click("Clear conversation").as_slice(),
            [Message::ChatClear]
        ));
        assert!(matches!(
            screen.click("× Close").as_slice(),
            [Message::ChatClose]
        ));
    }
    #[test]
    fn note_chat_is_labelled_and_exposes_note_actions() {
        let mut app = super::super::preview::test_app();
        app.chat_context = Some(crate::core::llm::Context::Note(uuid::Uuid::now_v7()));
        let snapshot = render(dialog(&app), &app.theme(), 850.0);
        assert!(snapshot.contains("Note chat"));
        assert!(!snapshot.contains("Task chat"));
        let snapshot = render(page(&app), &app.theme(), 1000.0);
        assert!(snapshot.contains("Allow chat to create, update and delete tasks and notes"));
    }
}
