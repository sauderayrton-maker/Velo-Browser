use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;
use glib::clone;

use crate::passwords::CredentialMeta;

#[derive(Clone)]
pub struct PasswordsPanel {
    pub root: gtk4::Box,
    list: gtk4::ListBox,
}

impl PasswordsPanel {
    pub fn build() -> Self {
        let list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(vec!["panel-list"])
            .build();

        let placeholder = gtk4::Label::builder()
            .label("No saved passwords")
            .css_classes(vec!["row-meta"])
            .margin_top(24)
            .build();
        list.set_placeholder(Some(&placeholder));

        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .child(&list)
            .vexpand(true)
            .build();

        let root = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .build();
        root.append(&scrolled);

        PasswordsPanel { root, list }
    }

    /// Reloads the list from the local credential index. Cheap — it's a
    /// small JSON file on disk, so this runs synchronously.
    pub fn refresh(&self) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }

        let mut entries = crate::passwords::list_credentials();
        entries.sort_by(|a, b| a.origin.cmp(&b.origin).then(a.username.cmp(&b.username)));

        for cred in entries {
            let row = make_row(&cred, &self.list);
            self.list.append(&row);
        }
    }
}

fn make_row(cred: &CredentialMeta, list: &gtk4::ListBox) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::builder()
        .css_classes(vec!["panel-row"])
        .activatable(false)
        .build();

    let hbox = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .margin_start(14)
        .margin_end(8)
        .margin_top(9)
        .margin_bottom(9)
        .spacing(4)
        .build();

    let body = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(3)
        .hexpand(true)
        .build();

    let host = cred.origin.split("://").nth(1).unwrap_or(&cred.origin);

    let title_lbl = gtk4::Label::builder()
        .label(host)
        .css_classes(vec!["row-title"])
        .halign(gtk4::Align::Start)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(30)
        .build();

    let user_lbl = gtk4::Label::builder()
        .label(&cred.username)
        .css_classes(vec!["row-meta"])
        .halign(gtk4::Align::Start)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(30)
        .build();

    let pass_lbl = gtk4::Label::builder()
        .label("••••••••")
        .css_classes(vec!["row-meta"])
        .halign(gtk4::Align::Start)
        .selectable(true)
        .build();

    body.append(&title_lbl);
    body.append(&user_lbl);
    body.append(&pass_lbl);

    let reveal_btn = gtk4::Button::builder()
        .icon_name("view-reveal-symbolic")
        .tooltip_text("Show password")
        .css_classes(vec!["flat", "row-delete-btn"])
        .valign(gtk4::Align::Center)
        .build();

    let copy_btn = gtk4::Button::builder()
        .icon_name("edit-copy-symbolic")
        .tooltip_text("Copy password")
        .css_classes(vec!["flat", "row-delete-btn"])
        .valign(gtk4::Align::Center)
        .build();

    let del_btn = gtk4::Button::builder()
        .icon_name("user-trash-symbolic")
        .tooltip_text("Delete")
        .css_classes(vec!["flat", "row-delete-btn"])
        .valign(gtk4::Align::Center)
        .build();

    let revealed = Rc::new(Cell::new(false));
    let origin = cred.origin.clone();
    let username = cred.username.clone();

    reveal_btn.connect_clicked(clone!(
        #[weak] pass_lbl,
        #[strong] revealed,
        #[strong] origin,
        #[strong] username,
        move |btn| {
            if revealed.get() {
                pass_lbl.set_label("••••••••");
                btn.set_icon_name("view-reveal-symbolic");
                btn.set_tooltip_text(Some("Show password"));
                revealed.set(false);
                return;
            }

            crate::passwords::get_credential(origin.clone(), username.clone(), clone!(
                #[weak] pass_lbl,
                #[weak] btn,
                #[strong] revealed,
                move |password| {
                    if let Some(pw) = password {
                        pass_lbl.set_label(&pw);
                        btn.set_icon_name("view-conceal-symbolic");
                        btn.set_tooltip_text(Some("Hide password"));
                        revealed.set(true);
                    }
                }
            ));
        }
    ));

    let origin = cred.origin.clone();
    let username = cred.username.clone();

    copy_btn.connect_clicked(clone!(
        #[strong] origin,
        #[strong] username,
        move |btn| {
            crate::passwords::get_credential(origin.clone(), username.clone(), clone!(
                #[weak] btn,
                move |password| {
                    let Some(pw) = password else { return };
                    if let Some(display) = gtk4::gdk::Display::default() {
                        display.clipboard().set_text(&pw);
                    }
                    btn.set_icon_name("object-select-symbolic");
                    glib::timeout_add_local_once(std::time::Duration::from_millis(900), clone!(
                        #[weak] btn,
                        move || btn.set_icon_name("edit-copy-symbolic")
                    ));
                }
            ));
        }
    ));

    let origin = cred.origin.clone();
    let username = cred.username.clone();

    del_btn.connect_clicked(clone!(
        #[weak] list,
        #[weak] row,
        move |_| {
            crate::passwords::delete_credential(origin.clone(), username.clone(), |_| {});
            list.remove(&row);
        }
    ));

    hbox.append(&body);
    hbox.append(&reveal_btn);
    hbox.append(&copy_btn);
    hbox.append(&del_btn);
    row.set_child(Some(&hbox));
    row
}
