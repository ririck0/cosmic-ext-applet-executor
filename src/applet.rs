use std::time::Duration;

use cosmic::iced::window::Id;
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::widget::rectangle_tracker::{RectangleTracker, RectangleUpdate, rectangle_tracker_subscription};

use crate::config::{APP_ID, BlockConfig, ExecutorConfig};

pub fn run() -> cosmic::iced::Result {
    cosmic::applet::run::<ExecutorApplet>(())
}

struct ExecutorApplet {
    core: cosmic::app::Core,
    config: ExecutorConfig,
    outputs: Vec<String>,
    ticks: Vec<u64>,
    popup: Option<Id>,
    rectangle_tracker: Option<RectangleTracker<u32>>,
    rectangle: cosmic::iced::Rectangle,
    edit_commands: Vec<String>,
    edit_intervals: Vec<String>,
    edit_separator: String,
    edit_font_size: f32,
}

fn execute_command(command: String) -> String {
    match std::process::Command::new("sh")
        .args(["-c", &command])
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            } else {
                format!("Error: {}", String::from_utf8_lossy(&output.stderr).trim())
            }
        }
        Err(e) => format!("Error: {e}"),
    }
}

fn parse_ansi(input: &str) -> Vec<(String, Option<cosmic::iced::Color>)> {
    use cosmic::iced::Color;

    let ansi_color = |code: u8| -> Option<Color> {
        match code {
            30 => Some(Color::from_rgb(0.0, 0.0, 0.0)),
            31 => Some(Color::from_rgb(0.9, 0.2, 0.2)),
            32 => Some(Color::from_rgb(0.2, 0.8, 0.2)),
            33 => Some(Color::from_rgb(0.95, 0.7, 0.1)),
            34 => Some(Color::from_rgb(0.3, 0.5, 1.0)),
            35 => Some(Color::from_rgb(0.8, 0.3, 0.8)),
            36 => Some(Color::from_rgb(0.2, 0.8, 0.8)),
            37 => Some(Color::from_rgb(0.9, 0.9, 0.9)),
            90 => Some(Color::from_rgb(0.5, 0.5, 0.5)),
            91 => Some(Color::from_rgb(1.0, 0.4, 0.4)),
            92 => Some(Color::from_rgb(0.4, 1.0, 0.4)),
            93 => Some(Color::from_rgb(1.0, 0.9, 0.3)),
            94 => Some(Color::from_rgb(0.5, 0.7, 1.0)),
            95 => Some(Color::from_rgb(1.0, 0.5, 1.0)),
            96 => Some(Color::from_rgb(0.4, 1.0, 1.0)),
            97 => Some(Color::WHITE),
            _ => None,
        }
    };

    let mut result = Vec::new();
    let mut current_color: Option<Color> = None;
    let mut remaining = input;

    while !remaining.is_empty() {
        if let Some(esc_start) = remaining.find('\x1b') {
            if esc_start > 0 {
                result.push((remaining[..esc_start].to_string(), current_color));
            }
            remaining = &remaining[esc_start..];

            if remaining.starts_with("\x1b[") {
                if let Some(m_pos) = remaining.find('m') {
                    let codes_str = &remaining[2..m_pos];
                    for code_str in codes_str.split(';') {
                        if let Ok(code) = code_str.trim().parse::<u8>() {
                            if code == 0 {
                                current_color = None;
                            } else if let Some(c) = ansi_color(code) {
                                current_color = Some(c);
                            }
                        }
                    }
                    remaining = &remaining[m_pos + 1..];
                    continue;
                }
            }
            remaining = &remaining[1..];
        } else {
            result.push((remaining.to_string(), current_color));
            break;
        }
    }

    result
}

fn save_config(config: &ExecutorConfig) {
    let path = dirs::config_dir()
        .unwrap_or_default()
        .join("cosmic/io.github.cosmic_utils.cosmic-ext-applet-executor/v1/config.json");

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let blocks: Vec<serde_json::Value> = config.blocks.iter().map(|b| {
        serde_json::json!({
            "command": b.command,
            "interval": b.interval,
        })
    }).collect();

    let mut obj = serde_json::json!({
        "separator": config.separator,
        "blocks": blocks,
    });

    if let Some(fs) = config.font_size {
        obj["font_size"] = serde_json::json!(fs);
    }

    let _ = std::fs::write(&path, serde_json::to_string_pretty(&obj).unwrap_or_default());
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Output(usize, String),
    TogglePopup,
    OpenPopupDelayed,
    Rectangle(RectangleUpdate<u32>),
    PopupClosed(Id),
    EditCommand(usize, String),
    EditInterval(usize, String),
    EditSeparator(String),
    IncrFontSize,
    DecrFontSize,
    AddBlock,
    RemoveBlock(usize),
    MoveBlockUp(usize),
    MoveBlockDown(usize),

    Save,
}

impl ExecutorApplet {
    fn sync_edit_state(&mut self) {
        self.edit_commands = self.config.blocks.iter().map(|b| b.command.clone()).collect();
        self.edit_intervals = self.config.blocks.iter().map(|b| b.interval.to_string()).collect();
        self.edit_separator = self.config.separator.clone();
        self.edit_font_size = self.config.font_size.unwrap_or(14.0);
    }
}

impl cosmic::Application for ExecutorApplet {
    type Flags = ();
    type Message = Message;
    type Executor = cosmic::SingleThreadExecutor;

    const APP_ID: &'static str = APP_ID;

    fn init(core: cosmic::app::Core, _flags: Self::Flags) -> (Self, cosmic::app::Task<Self::Message>) {
        let config = ExecutorConfig::config();
        let n = config.blocks.len();

        let applet = Self {
            core,
            outputs: vec![String::new(); n],
            ticks: vec![0u64; n],
            popup: None,
            rectangle_tracker: None,
            rectangle: cosmic::iced::Rectangle::default(),
            edit_commands: config.blocks.iter().map(|b| b.command.clone()).collect(),
            edit_intervals: config.blocks.iter().map(|b| b.interval.to_string()).collect(),

            edit_separator: config.separator.clone(),
            edit_font_size: config.font_size.unwrap_or(14.0),
            config,
        };

        let tasks: Vec<cosmic::app::Task<Message>> = applet.config.blocks.iter().enumerate()
            .map(|(i, block)| {
                let cmd = block.command.clone();
                cosmic::task::future(async move {
                    let output = tokio::task::spawn_blocking(move || execute_command(cmd))
                        .await
                        .unwrap_or_else(|e| format!("Error: {e}"));
                    Message::Output(i, output)
                })
            })
            .collect();

        (applet, cosmic::task::batch(tasks))
    }

    fn core(&self) -> &cosmic::app::Core { &self.core }
    fn core_mut(&mut self) -> &mut cosmic::app::Core { &mut self.core }

    fn on_close_requested(&self, id: cosmic::iced::window::Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Message> {
        cosmic::iced::Subscription::batch([
            cosmic::iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick),
            rectangle_tracker_subscription(0u32).map(|e| Message::Rectangle(e.1)),
        ])
    }

    fn update(&mut self, message: Message) -> cosmic::app::Task<Self::Message> {
        match message {
            Message::Tick => {
                self.config = ExecutorConfig::config();

                let n = self.config.blocks.len();
                self.outputs.resize(n, String::new());
                self.ticks.resize(n, 0);

                let tasks: Vec<cosmic::app::Task<Message>> = self.config.blocks.iter().enumerate()
                    .filter_map(|(i, block)| {
                        self.ticks[i] += 1;
                        if self.ticks[i] >= block.interval {
                            self.ticks[i] = 0;
                            let cmd = block.command.clone();
                            Some(cosmic::task::future(async move {
                                let output = tokio::task::spawn_blocking(move || execute_command(cmd))
                                    .await
                                    .unwrap_or_else(|e| format!("Error: {e}"));
                                Message::Output(i, output)
                            }))
                        } else {
                            None
                        }
                    })
                    .collect();

                cosmic::task::batch(tasks)
            }
            Message::Output(i, output) => {
                if i < self.outputs.len() {
                    self.outputs[i] = output;
                }
                cosmic::task::none()
            }
            Message::Rectangle(u) => {
                match u {
                    RectangleUpdate::Rectangle(r) => {
                        self.rectangle = r.1;
                    }
                    RectangleUpdate::Init(tracker) => {
                        self.rectangle_tracker = Some(tracker);
                    }
                }
                cosmic::task::none()
            }
            Message::TogglePopup => {
                if let Some(id) = self.popup.take() {
                    destroy_popup(id)
                } else {
                    self.sync_edit_state();
                    cosmic::task::future(async {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        Message::OpenPopupDelayed
                    })
                }
            }
            Message::OpenPopupDelayed => {
                if self.popup.is_some() {
                    return cosmic::task::none();
                }
                let new_id = cosmic::iced::window::Id::unique();
                self.popup = Some(new_id);
                let parent = self.core.main_window_id()
                    .unwrap_or(cosmic::iced::window::Id::RESERVED);
                let mut settings = self.core.applet.get_popup_settings(
                    parent, new_id, None, None, None,
                );
                let r = self.rectangle;
                settings.positioner.anchor_rect = cosmic::iced::Rectangle {
                    x: r.x.max(1.0) as i32,
                    y: r.y.max(1.0) as i32,
                    width: r.width.max(1.0) as i32,
                    height: r.height.max(1.0) as i32,
                };
                settings.positioner.size = Some((1500, 1));
                get_popup(settings)
            }
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
                cosmic::task::none()
            }
            Message::EditCommand(i, val) => {
                if i < self.edit_commands.len() {
                    self.edit_commands[i] = val;
                }
                cosmic::task::none()
            }
            Message::EditInterval(i, val) => {
                if i < self.edit_intervals.len() {
                    self.edit_intervals[i] = val;
                }
                cosmic::task::none()
            }
            Message::EditSeparator(val) => {
                self.edit_separator = val;
                cosmic::task::none()
            }
            Message::IncrFontSize => {
                self.edit_font_size = (self.edit_font_size + 1.0).min(72.0);
                cosmic::task::none()
            }
            Message::DecrFontSize => {
                self.edit_font_size = (self.edit_font_size - 1.0).max(6.0);
                cosmic::task::none()
            }
            Message::AddBlock => {
                self.edit_commands.push(String::new());
                self.edit_intervals.push("5".to_string());
                cosmic::task::none()
            }
            Message::RemoveBlock(i) => {
                if i < self.edit_commands.len() {
                    self.edit_commands.remove(i);
                    self.edit_intervals.remove(i);
                }
                cosmic::task::none()
            }
            Message::MoveBlockUp(i) => {
                if i > 0 && i < self.edit_commands.len() {
                    self.edit_commands.swap(i, i - 1);
                    self.edit_intervals.swap(i, i - 1);
                }
                cosmic::task::none()
            }
            Message::MoveBlockDown(i) => {
                if i + 1 < self.edit_commands.len() {
                    self.edit_commands.swap(i, i + 1);
                    self.edit_intervals.swap(i, i + 1);
                }
                cosmic::task::none()
            }

            Message::Save => {
                let blocks: Vec<BlockConfig> = self.edit_commands.iter().enumerate()
                    .filter(|(_, cmd)| !cmd.is_empty())
                    .map(|(i, cmd)| {
                        let interval = self.edit_intervals.get(i)
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(5);
                        BlockConfig { command: cmd.clone(), interval }
                    })
                    .collect();

                self.config = ExecutorConfig {
                    blocks,
                    separator: self.edit_separator.clone(),
                    font_size: Some(self.edit_font_size),
                };

                save_config(&self.config);

                let n = self.config.blocks.len();
                self.outputs.resize(n, String::new());
                self.ticks = vec![0u64; n];

                let tasks: Vec<cosmic::app::Task<Message>> = self.config.blocks.iter().enumerate()
                    .map(|(i, block)| {
                        let cmd = block.command.clone();
                        cosmic::task::future(async move {
                            let output = tokio::task::spawn_blocking(move || execute_command(cmd))
                                .await
                                .unwrap_or_else(|e| format!("Error: {e}"));
                            Message::Output(i, output)
                        })
                    })
                    .collect();

                cosmic::task::batch(tasks)
            }
        }
    }

    fn view(&self) -> cosmic::Element<'_, Message> {
        use cosmic::iced::advanced::text::Span;
        use cosmic::iced::Color;
        use cosmic::iced::widget::rich_text;
        use cosmic::widget::{autosize, button, row, Id};

        let sep = format!(" {} ", self.config.separator.trim());
        let font_size = self.config.font_size;

        let apply = |span: Span<'static, (), _>, color: Option<Color>| {
            let span = if let Some(c) = color { span.color(c) } else { span };
            if let Some(s) = font_size { span.size(s) } else { span }
        };

        let mut children: Vec<cosmic::Element<'_, ()>> = Vec::new();

        for (i, output) in self.outputs.iter().enumerate() {
            if i > 0 {
                let sep_spans = vec![apply(Span::new(sep.clone()), None)];
                let sep_el: cosmic::Element<'_, ()> = rich_text(sep_spans).into();
                children.push(sep_el);
            }

            let spans: Vec<Span<'static, (), _>> = parse_ansi(output)
                .into_iter()
                .map(|(txt, color)| apply(Span::new(txt), color))
                .collect();

            let text_el: cosmic::Element<'_, ()> = rich_text(spans).into();
            children.push(text_el);
        }

        let content = row::with_children(children)
            .align_y(cosmic::iced::Alignment::Center);

        let label: cosmic::Element<'_, ()> = content.into();
        let label = label.map(|_| Message::Tick);

        let btn = button::custom(label)
            .class(cosmic::theme::Button::AppletIcon)
            .on_press_down(Message::TogglePopup);

        let tracked = if let Some(tracker) = self.rectangle_tracker.as_ref() {
            cosmic::Element::from(tracker.container(0u32, btn).ignore_bounds(true))
        } else {
            btn.into()
        };

        autosize::autosize(tracked, Id::new("executor_applet")).into()
    }

    fn view_window(&self, _id: Id) -> cosmic::Element<'_, Message> {
        self.popup_view()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl ExecutorApplet {
    fn popup_view(&self) -> cosmic::Element<'_, Message> {
        use cosmic::widget::{button, column, list_column, row, settings, text, text_input};
        use cosmic::iced::Length;

        let mut blocks_col = list_column();

        let n = self.edit_commands.len();
        for (i, (cmd, interval)) in self.edit_commands.iter().zip(self.edit_intervals.iter()).enumerate() {
            let interval_val: u64 = interval.parse().unwrap_or(5);

            let mut up_btn = button::standard("↑");
            if i > 0 { up_btn = up_btn.on_press(Message::MoveBlockUp(i)); }

            let mut down_btn = button::standard("↓");
            if i + 1 < n { down_btn = down_btn.on_press(Message::MoveBlockDown(i)); }

            let interval_group = cosmic::widget::container(
                row::with_children(vec![
                    text(interval.as_str())
                        .width(Length::Fixed(28.0))
                        .align_x(cosmic::iced::Alignment::Center)
                        .into(),
                    button::standard("−")
                        .on_press(Message::EditInterval(i, interval_val.saturating_sub(1).to_string()))
                        .into(),
                    button::standard("+")
                        .on_press(Message::EditInterval(i, (interval_val + 1).to_string()))
                        .into(),
                ])
                .spacing(4)
                .align_y(cosmic::iced::Alignment::Center),
            )
            .style(|theme: &cosmic::Theme| {
                let c = theme.cosmic();
                cosmic::iced::widget::container::Style {
                    border: cosmic::iced::Border {
                        radius: c.corner_radii.radius_s.into(),
                        width: 1.0,
                        color: c.background.divider.into(),
                    },
                    ..Default::default()
                }
            })
            .padding([2, 4]);

            let row = row::with_children(vec![
                text_input("shell command...", cmd)
                    .on_input(move |v| Message::EditCommand(i, v))
                    .width(Length::Fill)
                    .into(),
                interval_group.into(),
                up_btn.into(),
                down_btn.into(),
                button::destructive("🗑")
                    .on_press(Message::RemoveBlock(i))
                    .into(),
            ])
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center);

            blocks_col = blocks_col.add(settings::item_row(vec![row.into()]));
        }

        let settings_col = column::with_children(vec![
            row::with_children(vec![
                text("Separator:").width(Length::Fixed(80.0)).into(),
                text_input("|", &self.edit_separator)
                    .on_input(Message::EditSeparator)
                    .width(Length::Fixed(30.0))
                    .into(),
            ])
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .into(),
            row::with_children(vec![
                text("Font size:").width(Length::Fixed(80.0)).into(),
                cosmic::widget::container(
                    row::with_children(vec![
                        text(format!("{:.0}", self.edit_font_size))
                            .width(Length::Fixed(32.0))
                            .align_x(cosmic::iced::Alignment::Center)
                            .into(),
                        button::standard("−")
                            .on_press(Message::DecrFontSize)
                            .into(),
                        button::standard("+")
                            .on_press(Message::IncrFontSize)
                            .into(),
                    ])
                    .spacing(4)
                    .align_y(cosmic::iced::Alignment::Center),
                )
                .style(|theme: &cosmic::Theme| {
                    let c = theme.cosmic();
                    cosmic::iced::widget::container::Style {
                        border: cosmic::iced::Border {
                            radius: c.corner_radii.radius_s.into(),
                            width: 1.0,
                            color: c.background.divider.into(),
                        },
                        ..Default::default()
                    }
                })
                .padding([2, 4])
                .into(),
            ])
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .into(),
        ])
        .spacing(4);

        let left_col = column::with_children(vec![
            button::standard("+")
                .on_press(Message::AddBlock)
                .into(),
            settings_col.into(),
        ])
        .spacing(8);

        let bottom_row = row::with_children(vec![
            left_col.into(),
            cosmic::widget::space::horizontal().into(),
            button::suggested("Save")
                .on_press(Message::Save)
                .into(),
        ])
        .spacing(8)
        .align_y(cosmic::iced::Alignment::Start);

        let content = column::with_children(vec![
            text("Cosmic Executor")
                .size(18)
                .width(Length::Fill)
                .align_x(cosmic::iced::Alignment::Center)
                .into(),
            text("Command | Interval in seconds:")
                .width(Length::Fill)
                .align_x(cosmic::iced::Alignment::Center)
                .into(),
            blocks_col.into(),
            bottom_row.into(),
        ])
        .spacing(12)
        .padding(16)
        .width(cosmic::iced::Length::Fixed(700.0));

        {
            use cosmic::iced::{Color, Shadow};
            use cosmic::iced::alignment::{Horizontal, Vertical};
            use cosmic::iced::{Length, Limits};
            use cosmic::iced::widget::container::Style;
            use cosmic::widget::autosize::autosize;
            use cosmic::applet::cosmic_panel_config::PanelAnchor;

            let (vertical_align, horizontal_align) = match self.core.applet.anchor {
                PanelAnchor::Left => (Vertical::Center, Horizontal::Left),
                PanelAnchor::Right => (Vertical::Center, Horizontal::Right),
                PanelAnchor::Top => (Vertical::Top, Horizontal::Center),
                PanelAnchor::Bottom => (Vertical::Bottom, Horizontal::Center),
            };

            autosize(
                cosmic::widget::container(
                    cosmic::widget::container(content).style(|theme: &cosmic::Theme| {
                        let c = theme.cosmic();
                        Style {
                            text_color: Some(c.background.on.into()),
                            background: Some(Color::from(c.background.base).into()),
                            border: cosmic::iced::Border {
                                radius: c.corner_radii.radius_m.into(),
                                width: 1.0,
                                color: c.background.divider.into(),
                            },
                            shadow: Shadow::default(),
                            icon_color: Some(c.background.on.into()),
                            ..Default::default()
                        }
                    }),
                )
                .width(Length::Shrink)
                .height(Length::Shrink)
                .align_x(horizontal_align)
                .align_y(vertical_align),
                cosmic::iced::id::Id::new("cosmic-applet-executor-autosize"),
            )
            .limits(
                Limits::NONE
                    .min_height(1.)
                    .min_width(360.0)
                    .max_width(800.0)
                    .max_height(1000.0),
            )
            .into()
        }
    }
}
