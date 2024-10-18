use crossterm::{
	event::{
		self, Event,
		KeyCode::{self, Char},
		KeyEvent,
	},
	execute,
	terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use git2::{Repository, Revwalk};
use std::{
	error::Error,
	io::{self, Stdout},
};
use tui::{
	backend::CrosstermBackend,
	layout::{Constraint, Direction, Layout, Rect, Size},
	style::{Color, Style, Stylize as _},
	text::{Line, Span, Text, ToSpan as _},
	widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
	Frame, Terminal,
};

use crate::git::{next_commit, CommitInfo};

pub struct App<'repo> {
	repo: &'repo Repository,
	commit_infos: Vec<CommitInfo<'repo>>,
	revwalk: Revwalk<'repo>,
	log_mode: LogMode,
	log_state: ListState,
	popup: Option<Text<'static>>,
}

impl App<'_> {
	pub fn new<'a>(repo: &'a Repository, revwalk: Revwalk<'a>) -> App<'a> {
		App {
			repo,
			commit_infos: vec![],
			revwalk,
			log_mode: LogMode::Short,
			log_state: ListState::default(),
			popup: None,
		}
	}
}

#[derive(PartialEq)]
enum LogMode {
	Short,
	Medium,
	Long,
}

type CrosstermTerm = Terminal<CrosstermBackend<Stdout>>;

pub fn setup() -> Result<CrosstermTerm, Box<dyn Error>> {
	enable_raw_mode()?;
	let mut stdout = io::stdout();
	execute!(stdout, EnterAlternateScreen)?;
	let backend = CrosstermBackend::new(stdout);
	Ok(Terminal::new(backend)?)
}

pub fn teardown(terminal: &mut CrosstermTerm) {
	_ = disable_raw_mode();
	_ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
	_ = terminal.show_cursor();
}

pub fn run_app(terminal: &mut CrosstermTerm, mut app: App) -> Result<(), Box<dyn Error>> {
	loop {
		let needed: usize = usize::from(terminal.size()?.height / 2) + app.log_state.offset();
		while app.commit_infos.len() < needed {
			let commit_info = match next_commit(app.repo, &mut app.revwalk) {
				Ok(None) => break,
				Ok(Some(ci)) => ci,
				Err(err) => {
					app.popup = Some(err.message().to_owned().into());
					break;
				},
			};
			app.commit_infos.push(commit_info);
		}

		terminal.draw(|frame| ui(frame, &mut app))?;
		if let Event::Key(key) = event::read()? {
			match handle_input(&key, &mut app, &terminal.size()?) {
				Ok(false) => {
					return Ok(());
				},
				Ok(true) => {}, // ignored
				Err(err) => app.popup = Some(format!("{}", err).into()),
			}
		}
	}
}

// returns whether to continue running the app
fn handle_input(key: &KeyEvent, app: &mut App, term_size: &Size) -> Result<bool, Box<dyn Error>> {
	if app.popup.is_some() {
		// clear the popup on any key press
		app.popup = None;
		return Ok(true);
	}

	match key {
		// scroll
		KeyEvent {
			code: Char('j') | KeyCode::Down,
			..
		} => scroll(app, 1),
		KeyEvent {
			code: Char('k') | KeyCode::Up,
			..
		} => scroll(app, -1),
		KeyEvent { code: Char('d'), .. }
		| KeyEvent {
			code: KeyCode::PageDown,
			..
		} => scroll(app, (term_size.height / 2).try_into().unwrap()),
		KeyEvent { code: Char('u'), .. }
		| KeyEvent {
			code: KeyCode::PageUp, ..
		} => scroll(app, -i16::try_from(term_size.height / 2).unwrap()),
		KeyEvent { code: Char('g'), .. }
		| KeyEvent {
			code: KeyCode::Home, ..
		} => {
			// TODO
		},
		KeyEvent { code: Char('G'), .. } | KeyEvent { code: KeyCode::End, .. } => {
			// TODO
		},
		// other interactions
		KeyEvent { code: Char('1'), .. } => {
			app.log_mode = LogMode::Short;
		},
		KeyEvent { code: Char('2'), .. } => {
			app.log_mode = LogMode::Medium;
		},
		KeyEvent { code: Char('3'), .. } => {
			app.log_mode = LogMode::Long;
		},
		KeyEvent { code: Char('h'), .. } => app.popup = Some(make_help_text()),
		KeyEvent {
			code: Char('q') | KeyCode::Esc,
			..
		} => {
			return Ok(false);
		},
		_ => {}, // ignored
	};
	Ok(true)
}

fn scroll(app: &mut App, amount: i16) {
	match app.log_state.selected() {
		None => app.log_state.select(Some(0)),
		Some(index) => {
			let new_index = index.saturating_add_signed(amount.into());
			app.log_state.select(Some(new_index.clamp(0, app.commit_infos.len() - 1)));
		},
	}
}

fn make_help_text() -> Text<'static> {
	let mut help = vec!["h           this help", "q  esc      close window"];
	(help.drain(..).map(Line::from).collect::<Vec<_>>()).into()
}

fn ui(frame: &mut Frame, app: &mut App) {
	let area = Rect::new(
		frame.area().x,
		frame.area().y,
		frame.area().width,
		frame.area().height - 1,
	);

	let commit_list = List::new(app.commit_infos.iter().map(|ci| commit_info_to_item(ci, &app.log_mode)))
		.highlight_style(Style::default().bg(Color::Indexed(237))); // 232 is black, 255 is white; 237 is dark gray
	frame.render_stateful_widget(commit_list, area, &mut app.log_state);

	if let Some(popup) = &app.popup {
		let paragraph = Paragraph::new(popup.clone()).wrap(Wrap { trim: false });
		let area = centered_rect(80, 80, frame.area());
		frame.render_widget(Clear, area);
		frame.render_widget(Block::default().borders(Borders::all()), area);
		frame.render_widget(
			paragraph,
			area.inner(tui::layout::Margin {
				vertical: 2,
				horizontal: 3,
			}),
		);
	}
}

fn commit_info_to_item<'a>(ci: &'a CommitInfo, log_mode: &LogMode) -> ListItem<'a> {
	let mut commit_id = ci.commit_id.to_string();
	commit_id.truncate(8);
	let mut lines = vec![Line::from(vec![
		Span::from(commit_id).yellow(),
		" ".to_span(),
		ci.author.to_span().light_blue(),
	])];
	match log_mode {
		LogMode::Short => lines.push(Line::from(format!("    {}", ci.summary))),
		LogMode::Medium | LogMode::Long => {
			ci.message.lines().for_each(|l| lines.push(Line::from(format!("    {}", l))));
			lines.push(Line::from(""));
		},
	}
	if *log_mode == LogMode::Long {
		lines.extend(ci.stats.iter().map(|sl: &String| Line::from(sl.clone())));
		lines.push(Line::from(""));
	}
	return lines.into();
}

// from https://github.com/tui-rs-revival/ratatui/blob/main/examples/popup.rs
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
	let popup_layout = Layout::default()
		.direction(Direction::Vertical)
		.constraints(
			[
				Constraint::Percentage((100 - percent_y) / 2),
				Constraint::Percentage(percent_y),
				Constraint::Percentage((100 - percent_y) / 2),
			]
			.as_ref(),
		)
		.split(r);

	Layout::default()
		.direction(Direction::Horizontal)
		.constraints(
			[
				Constraint::Percentage((100 - percent_x) / 2),
				Constraint::Percentage(percent_x),
				Constraint::Percentage((100 - percent_x) / 2),
			]
			.as_ref(),
		)
		.split(popup_layout[1])[1]
}
