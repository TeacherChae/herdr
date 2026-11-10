use super::*;
use crate::input::TerminalKey;
use crate::raw_input::RawInputEvent;

fn ctrl_h_state(
    passthrough: bool,
    alternate_screen: bool,
    agent: Option<&str>,
) -> ClientShellState {
    let mut config = Config::default();
    config.keys.focus_pane_left =
        crate::config::BindingConfig::Many(vec!["prefix+h".to_owned(), "ctrl+h".to_owned()]);
    config
        .experimental
        .pass_direct_pane_focus_to_fullscreen_apps = passthrough;
    let mut projected = snapshot();
    if let Some(agent) = agent {
        projected.agents.push(ClientShellAgent {
            pane_id: "pane_1".into(),
            workspace_id: "ws_1".into(),
            tab_id: "tab_1".into(),
            name: None,
            display_agent: Some(agent.into()),
            agent: Some(agent.into()),
            title: None,
            terminal_title: None,
            terminal_title_stripped: None,
            agent_status: AgentStatus::Idle,
            state_change_seq: 0,
            state_labels: Vec::new(),
            tokens: Vec::new(),
            focused: true,
        });
    }
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(projected));
    let mut frame = surface();
    for pane in &mut frame.panes {
        pane.alternate_screen_active = alternate_screen;
    }
    state.set_pane_surface(frame);
    state
}

fn press_ctrl_h(state: &mut ClientShellState) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Key(TerminalKey::new(
        KeyCode::Char('h'),
        KeyModifiers::CONTROL,
    ))])
}

fn moved_focus(outcome: &ClientShellInput) -> bool {
    !(reached_pane(outcome) || outcome.requests.is_empty() && outcome.actions.is_empty())
}

fn reached_pane(outcome: &ClientShellInput) -> bool {
    outcome
        .requests
        .iter()
        .any(|request| matches!(request, ClientMessage::ClientShellPaneInput { .. }))
}

#[test]
fn direct_focus_key_moves_focus_on_the_normal_screen() {
    let mut state = ctrl_h_state(true, false, None);
    let outcome = press_ctrl_h(&mut state);
    assert!(moved_focus(&outcome), "{:?}", outcome.actions);
}

#[test]
fn direct_focus_key_reaches_a_fullscreen_app() {
    let mut state = ctrl_h_state(true, true, None);
    let outcome = press_ctrl_h(&mut state);
    assert!(reached_pane(&outcome));
}

#[test]
fn direct_focus_key_still_moves_focus_from_a_fullscreen_agent() {
    let mut state = ctrl_h_state(true, true, Some("codex"));
    let outcome = press_ctrl_h(&mut state);
    assert!(moved_focus(&outcome), "{:?}", outcome.actions);
}

#[test]
fn direct_focus_key_ignores_fullscreen_apps_when_disabled() {
    let mut state = ctrl_h_state(false, true, None);
    let outcome = press_ctrl_h(&mut state);
    assert!(moved_focus(&outcome), "{:?}", outcome.actions);
}

#[test]
fn prefix_focus_key_moves_focus_even_in_a_fullscreen_app() {
    let mut state = ctrl_h_state(true, true, None);
    let _ = state.handle_input_bytes(&[0x02]);
    assert_eq!(state.mode, ClientShellMode::Prefix);
    let outcome = state.handle_input_bytes(b"h");
    assert!(moved_focus(&outcome), "{:?}", outcome.actions);
}

#[test]
fn input_source_command_reads_and_reloads_config() {
    let mut config = Config::default();
    config.experimental.ascii_input_source_command = Some("  ".to_owned());
    let shell = ClientShellConfig::from_config(&config);
    assert_eq!(shell.ascii_input_source_command, None);

    config.experimental.ascii_input_source_command = Some(" winime-en.sh ".to_owned());
    let mut shell = ClientShellConfig::from_config(&config);
    assert_eq!(
        shell.ascii_input_source_command.as_deref(),
        Some("winime-en.sh")
    );

    shell.apply_live_config(&Config::default(), &[], &["experimental".to_owned()]);
    assert_eq!(
        shell.ascii_input_source_command.as_deref(),
        Some("winime-en.sh")
    );
    shell.apply_live_config(&Config::default(), &[], &[]);
    assert_eq!(shell.ascii_input_source_command, None);
}

#[cfg(unix)]
#[test]
fn input_source_command_runs_on_prefix_entry_only() {
    let marker =
        std::env::temp_dir().join(format!("herdr-input-source-command-{}", std::process::id()));
    let _ = std::fs::remove_file(&marker);
    let mut config = Config::default();
    config.experimental.switch_ascii_input_source_in_prefix = true;
    config.experimental.ascii_input_source_command =
        Some(format!("echo on >> '{}'", marker.display()));
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(snapshot()));
    assert!(state.uses_input_source_command());

    let _ = state.handle_input_bytes(&[0x02]);
    let changes = state.take_input_source_changes();
    assert_eq!(changes, vec![true]);
    for active in changes {
        state.run_input_source_command(active);
    }
    let _ = state.handle_input_bytes(&[0x1b]);
    let changes = state.take_input_source_changes();
    assert_eq!(changes, vec![false]);
    for active in changes {
        state.run_input_source_command(active);
    }
    for child in &mut state.input_source_command_children {
        let _ = child.wait();
    }
    let written = std::fs::read_to_string(&marker).unwrap_or_default();
    let _ = std::fs::remove_file(&marker);
    assert_eq!(written, "on\n");
}
