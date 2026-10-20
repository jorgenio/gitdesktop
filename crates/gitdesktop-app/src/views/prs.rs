//! Pestaña «PRs»: listado con checks de CI, detalle, creación y checkout.

use iced::widget::{button, center, column, container, row, scrollable, text, text_editor, text_input};
use iced::{Alignment, Element, Fill, Length};

use crate::github::{self, CheckState};
use crate::messages::Message;
use crate::state::App;
use crate::theme;

/// Vista lateral: lista de PRs abiertos + botón de creación.
pub fn list(app: &App) -> Element<'_, Message> {
    if !github::available() {
        return center(
            column![
                text("Requiere GitHub CLI").size(16),
                text("Instala y autentica `gh` para ver los Pull Requests:\ngh auth login")
                    .size(12)
                    .style(theme::muted),
            ]
            .spacing(6)
            .align_x(Alignment::Center),
        )
        .into();
    }

    let create = button(
        text("+ Nuevo Pull Request")
            .size(12)
            .align_x(Alignment::Center)
            .width(Fill),
    )
    .on_press(Message::CreatePrModalToggled(true))
    .padding([7, 0])
    .width(Fill)
    .style(button::primary);

    let mut list = column![].spacing(2).padding([6, 6]);

    if !app.prs_loaded {
        list = list.push(
            text("Cargando PRs…")
                .size(12)
                .style(theme::muted)
                .width(Fill),
        );
    } else if app.prs.is_empty() {
        list = list.push(
            text("Sin Pull Requests abiertos.")
                .size(12)
                .style(theme::muted)
                .width(Fill),
        );
    }

    for pr in &app.prs {
        let is_selected = app.selected_pr == Some(pr.number);
        let (check_glyph, check_style): (
            &str,
            fn(&iced::Theme) -> iced::widget::text::Style,
        ) = match pr.checks {
            CheckState::Success => ("✓ CI", theme::notice),
            CheckState::Failure => ("✗ CI", theme::error),
            CheckState::Pending => ("● CI", theme::muted),
            CheckState::None => ("—", theme::muted),
        };

        list = list.push(
            container(
                button(
                    column![
                        row![
                            text(format!("#{}", pr.number))
                                .size(12)
                                .style(theme::muted),
                            text(pr.title.clone())
                                .size(13)
                                .width(Fill),
                        ]
                        .spacing(6),
                        row![
                            text(check_glyph).size(11).style(check_style),
                            text(if pr.draft {
                                format!("borrador · {}", pr.author)
                            } else {
                                pr.author.clone()
                            })
                            .size(11)
                            .style(theme::muted),
                        ]
                        .spacing(6),
                    ]
                    .spacing(2)
                    .width(Fill),
                )
                .on_press(Message::PrSelected(Some(pr.number)))
                .padding([6, 8])
                .width(Fill)
                .style(theme::ghost),
            )
            .width(Fill)
            .style(theme::file_row(is_selected)),
        );
    }

    column![
        container(create).padding([6, 8]),
        scrollable(list).height(Fill),
    ]
    .height(Fill)
    .into()
}

/// Vista principal: detalle del PR seleccionado.
pub fn detail(app: &App) -> Element<'_, Message> {
    let Some(pr) = app
        .prs
        .iter()
        .find(|pr| Some(pr.number) == app.selected_pr)
    else {
        return center(
            column![
                text("Pull Requests").size(18),
                text("Selecciona un PR de la lista para ver su detalle.")
                    .size(12)
                    .style(theme::muted),
            ]
            .spacing(6)
            .align_x(Alignment::Center),
        )
        .into();
    };

    let slug = app.repo_path().and_then(|path| github::repo_slug(&path));
    let url = github::pr_url(slug, pr.number);
    let current_branch = app
        .repo
        .as_ref()
        .map(|open| open.status.branch.clone())
        .unwrap_or_default();

    let (check_text, check_style): (
        &str,
        fn(&iced::Theme) -> iced::widget::text::Style,
    ) = match pr.checks {
        CheckState::Success => ("✓ CI en verde", theme::notice),
        CheckState::Failure => ("✗ CI fallando", theme::error),
        CheckState::Pending => ("● CI en progreso", theme::muted),
        CheckState::None => ("Sin checks de CI", theme::muted),
    };

    let header = container(
        column![
            row![
                text(format!("#{} {}", pr.number, pr.title))
                    .size(17)
                    .width(Fill),
                button(text("Abrir en navegador").size(11).style(theme::muted))
                    .on_press_maybe(url.clone().map(Message::OpenUrl))
                    .padding([4, 8])
                    .style(theme::menu_item),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            row![
                text(check_text).size(12).style(check_style),
                text("·").size(12).style(theme::muted),
                text(format!(
                    "{} → {} · {}{}",
                    pr.head,
                    "base",
                    pr.author,
                    if pr.draft { " · BORRADOR" } else { "" }
                ))
                .size(12)
                .style(theme::muted),
            ]
            .spacing(6),
        ]
        .spacing(6),
    )
    .padding([12, 14])
    .width(Fill)
    .style(move |t| theme::file_row(true)(t));

    let actions = container(
        row![
            dialog_button(
                "Cambiar a esta rama",
                Message::CheckoutPr { number: pr.number },
                button::primary,
                pr.head != current_branch,
            ),
        ]
        .spacing(8),
    )
    .padding([10, 14])
    .width(Fill);

    column![header, actions].width(Fill).height(Fill).into()
}

/// Botón de diálogo reutilizable (versión local de esta vista).
fn dialog_button<'a>(
    label: &'static str,
    message: Message,
    style: fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style,
    enabled: bool,
) -> Element<'a, Message> {
    button(
        text(label)
            .size(13)
            .align_x(Alignment::Center)
            .width(Fill),
    )
    .on_press_maybe(enabled.then_some(message))
    .padding([8, 0])
    .width(Fill)
    .style(style)
    .into()
}

/// Modal de creación de PR (título + descripción).
pub fn create_modal(app: &App) -> Element<'_, Message> {
    let can_create = !app.pr_title.trim().is_empty() && !app.loading;

    let card = container(
        column![
            text("Nuevo Pull Request").size(18),
            text("Se creará desde tu rama actual hacia la rama base.")
                .size(12)
                .style(theme::muted),
            text_input("Título del PR", &app.pr_title)
                .on_input(Message::PrTitleChanged)
                .size(13)
                .padding([8, 10]),
            text_editor(&app.pr_body)
                .on_action(Message::PrBodyEdited)
                .size(12)
                .height(Length::Fixed(120.0)),
            row![
                dialog_button(
                    "Cancelar",
                    Message::CreatePrModalToggled(false),
                    button::text,
                    true,
                ),
                dialog_button(
                    "Crear PR",
                    Message::CreatePrPressed,
                    button::primary,
                    can_create,
                ),
            ]
            .spacing(10),
        ]
        .spacing(12)
        .max_width(480),
    )
    .padding(24)
    .style(theme::panel);

    iced::widget::Stack::with_children(vec![
        container(space_scrim()).width(Fill).height(Fill).into(),
        center(card).width(Fill).height(Fill).into(),
    ])
    .into()
}

fn space_scrim() -> iced::Element<'static, Message> {
    use iced::widget::space;
    space::Space::new().width(Fill).height(Fill).into()
}
