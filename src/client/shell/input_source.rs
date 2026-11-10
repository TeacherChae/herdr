use super::*;

impl ClientShellState {
    fn wants_ascii_input(&self) -> bool {
        if let Some(overlay) = self.overlay.as_ref() {
            return matches!(
                overlay,
                ClientShellOverlay::ConfirmClose(_)
                    | ClientShellOverlay::Help(_)
                    | ClientShellOverlay::Navigator(_)
                    | ClientShellOverlay::WorktreeRemove(_)
                    | ClientShellOverlay::ContextMenu(_)
                    | ClientShellOverlay::GlobalMenu(_)
            );
        }
        matches!(
            self.mode,
            ClientShellMode::Prefix
                | ClientShellMode::Navigate
                | ClientShellMode::Resize
                | ClientShellMode::Copy
        )
    }

    pub(crate) fn reconcile_input_source(&mut self) {
        // Keep the platform restore token while another window has focus. Restoring
        // through a global key injection is only safe after this client regains focus.
        if self.outer_focused == Some(false) {
            return;
        }
        let desired = self.config.switch_ascii_input_source_in_prefix && self.wants_ascii_input();
        if desired != self.ascii_input_source_active {
            self.ascii_input_source_active = desired;
            self.pending_input_source_changes.push(desired);
        }
    }

    pub(crate) fn take_input_source_changes(&mut self) -> Vec<bool> {
        std::mem::take(&mut self.pending_input_source_changes)
    }

    /// Whether ASCII input-source changes run a configured command instead of
    /// the platform switch.
    pub(crate) fn uses_input_source_command(&self) -> bool {
        self.config.ascii_input_source_command.is_some()
    }

    /// Run the configured command for one input-source change: the switch
    /// command when entering ASCII input, the restore command when leaving.
    pub(crate) fn run_input_source_command(&mut self, active: bool) {
        self.input_source_command_children
            .retain_mut(|child| matches!(child.try_wait(), Ok(None)));
        let command = if active {
            self.config.ascii_input_source_command.as_deref()
        } else {
            self.config.ascii_input_source_restore_command.as_deref()
        };
        let Some(command) = command else {
            return;
        };
        let mut process = crate::platform::detached_custom_command_process(command);
        process
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        match process.spawn() {
            Ok(child) => self.input_source_command_children.push(child),
            Err(error) => {
                tracing::warn!(%error, active, "failed to run input source command");
            }
        }
    }
}
