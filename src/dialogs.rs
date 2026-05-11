//! Confirmation and lifecycle prompt dialogs for the desktop shell.

use std::sync::Arc;

use gpui::prelude::*;
use gpui::{App, AsyncApp, SharedString, Window, div};
use gpui_component::{
    ActiveTheme, WindowExt,
    button::{Button, ButtonVariants},
    dialog::{Dialog, DialogFooter},
    notification::NotificationType,
    v_flex,
};

#[cfg(feature = "auto_update")]
use semver::Version;

use crate::startup::install;
#[cfg(feature = "auto_update")]
use crate::startup::update;

use super::ChessApp;

type ConfirmCallback = dyn Fn(&mut Window, &mut App);

#[derive(Clone)]
pub struct ConfirmDialogOptions {
    title: SharedString,
    message: SharedString,
    confirm_label: SharedString,
    cancel_label: SharedString,
}

impl ConfirmDialogOptions {
    pub fn new(
        title: impl Into<SharedString>,
        message: impl Into<SharedString>,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            confirm_label: SharedString::from("Confirm"),
            cancel_label: SharedString::from("Cancel"),
        }
    }

    pub fn confirm_label(
        mut self,
        label: impl Into<SharedString>,
    ) -> Self {
        self.confirm_label = label.into();
        self
    }

    pub fn cancel_label(
        mut self,
        label: impl Into<SharedString>,
    ) -> Self {
        self.cancel_label = label.into();
        self
    }
}

impl ChessApp {
    pub fn show_confirm_dialog<F>(
        window: &mut Window,
        cx: &mut App,
        options: ConfirmDialogOptions,
        on_confirm: F,
    ) where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        let on_confirm: Arc<ConfirmCallback> = Arc::new(on_confirm);

        window.open_dialog(cx, move |dlg: Dialog, _wnd, app: &mut App| {
            let title = options.title.clone();
            let message = options.message.clone();
            let confirm_label = options.confirm_label.clone();
            let cancel_label = options.cancel_label.clone();
            let on_confirm = on_confirm.clone();

            dlg.title(div().child(title))
                .child(
                    v_flex().gap_3().child(
                        div()
                            .text_sm()
                            .text_color(app.theme().muted_foreground)
                            .child(message),
                    ),
                )
                .footer(DialogFooter::new().children(vec![
                    Button::new("confirm-dialog-cancel")
                        .label(cancel_label)
                        .cursor_pointer()
                        .on_click(|_, window, cx| {
                            window.close_dialog(cx);
                        })
                        .flex_1(),
                    Button::new("confirm-dialog-confirm")
                        .primary()
                        .cursor_pointer()
                        .label(confirm_label)
                        .on_click(move |_, window, cx| {
                            window.close_dialog(cx);
                            on_confirm(window, cx);
                        })
                        .flex_1(),
                ]))
        });
    }

    pub fn show_install_prompt(
        window: &mut Window,
        cx: &mut App,
    ) {
        window.open_dialog(cx, move |dlg: Dialog, _wnd, app: &mut App| {
            dlg.title(div().child("Install Antarian Chess"))
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            div()
                                .text_sm()
                                .text_color(app.theme().muted_foreground)
                                .child(
                                    "This app is running from a temporary location. Install it to a permanent location and create shortcuts?",
                                ),
                        ),
                )
                .footer(DialogFooter::new().children(vec![
                    Button::new("install-prompt-skip")
                        .label("No Thanks")
                        .cursor_pointer()
                        .on_click(move |_, window, cx| {
                            window.close_dialog(cx);
                        })
                        .flex_1(),
                    Button::new("install-prompt-confirm")
                        .primary()
                        .cursor_pointer()
                        .label("Install")
                        .on_click(move |_, window, cx| {
                            let window_handle = window.window_handle();
                            let bg = cx.background_executor().clone();
                            let task = bg.spawn(async move { install::attempt_self_install() });
                            window.close_dialog(cx);
                            window.push_notification(
                                (
                                    NotificationType::Info,
                                    SharedString::from("Installing Antarian Chess..."),
                                ),
                                cx,
                            );
                            cx.spawn(async move |async_app: &mut AsyncApp| {
                                let result = task.await;
                                async_app.update(|cx| {
                                    let _ = cx.update_window(window_handle, |_, window, app| {
                                        if let Err(error) = result {
                                            window.push_notification(
                                                (
                                                    NotificationType::Error,
                                                    SharedString::from(format!(
                                                        "Could not install app: {}",
                                                        error
                                                    )),
                                                ),
                                                app,
                                            );
                                        }
                                    });
                                });
                            })
                            .detach();
                        })
                        .flex_1(),
                ]))
        });
    }

    #[cfg(feature = "auto_update")]
    pub fn show_update_prompt(
        window: &mut Window,
        cx: &mut App,
        version: Version,
    ) {
        window.open_dialog(cx, move |dlg: Dialog, _wnd, app: &mut App| {
            let version_for_install = version.clone();

            dlg.title(div().child("Update Antarian Chess"))
                .child(
                    v_flex().gap_3().child(
                        div()
                            .text_sm()
                            .text_color(app.theme().muted_foreground)
                            .child(format!(
                                "Antarian Chess {} is available. Download and restart now?",
                                version
                            )),
                    ),
                )
                .footer(DialogFooter::new().children(vec![
                    Button::new("update-prompt-skip")
                        .label("Later")
                        .cursor_pointer()
                        .on_click(|_, window, cx| {
                            window.close_dialog(cx);
                        })
                        .flex_1(),
                    Button::new("update-prompt-confirm")
                        .primary()
                        .cursor_pointer()
                        .label("Update")
                        .on_click(move |_, window, cx| {
                            let window_handle = window.window_handle();
                            let version_for_task = version_for_install.clone();
                            let bg = cx.background_executor().clone();
                            let task = bg.spawn(async move {
                                update::download_update(&version_for_task)
                            });
                            window.close_dialog(cx);
                            window.push_notification(
                                (
                                    NotificationType::Info,
                                    SharedString::from("Downloading update..."),
                                ),
                                cx,
                            );
                            cx.spawn(async move |async_app: &mut AsyncApp| {
                                let result = task.await;
                                async_app.update(|cx| {
                                    let _ = cx.update_window(window_handle, |_, window, app| {
                                        if let Err(error) = result {
                                            window.push_notification(
                                                (
                                                    NotificationType::Error,
                                                    SharedString::from(format!(
                                                        "Could not update app: {}",
                                                        error
                                                    )),
                                                ),
                                                app,
                                            );
                                        }
                                    });
                                });
                            })
                            .detach();
                        })
                        .flex_1(),
                ]))
        });
    }
}
