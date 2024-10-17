use crossterm::{
	event::{
		self, Event,
		KeyCode::{self, Char},
		KeyEvent,
	},
	execute,
	terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use git2::{Oid, Repository, Revwalk};
use std::{
	error::Error,
	io::{self, Stdout},
};
use tui::{
	backend::CrosstermBackend,
	layout::{Constraint, Direction, Layout, Rect, Size},
	text::{Line, Text},
	widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
	Frame, Terminal,
};

use crate::git::{next_commit, CommitInfo};

pub struct App<'a> {
	repo: &'a Repository,
	commit_id: Oid,
	commit_infos: Vec<CommitInfo>,
	revwalk: Revwalk<'a>,
	log_state: ListState,
	popup: Option<Text<'static>>,
}

impl App<'_> {
	pub fn new<'a>(repo: &'a Repository, commit_id: Oid, revwalk: Revwalk<'a>) -> App<'a> {
		App {
			repo,
			commit_id,
			commit_infos: vec![],
			revwalk,
			log_state: ListState::default(),
			popup: None,
		}
	}
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
		terminal.draw(|frame| ui(frame, &mut app))?;
		if let Event::Key(key) = event::read()? {
			match handle_input(&key, &mut app, &terminal.size()?) {
				Ok(false) => {
					return Ok(());
				}
				Ok(true) => {} // ignored
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
		} => scroll(app, term_size, 1),
		KeyEvent {
			code: Char('k') | KeyCode::Up,
			..
		} => scroll(app, term_size, -1),
		KeyEvent { code: Char('d'), .. }
		| KeyEvent {
			code: KeyCode::PageDown,
			..
		} => scroll(app, term_size, (term_size.height / 2).try_into().unwrap()),
		KeyEvent { code: Char('u'), .. }
		| KeyEvent {
			code: KeyCode::PageUp, ..
		} => scroll(app, term_size, -i16::try_from(term_size.height / 2).unwrap()),
		KeyEvent { code: Char('g'), .. }
		| KeyEvent {
			code: KeyCode::Home, ..
		} => {
			// TODO
		}
		KeyEvent { code: Char('G'), .. } | KeyEvent { code: KeyCode::End, .. } => {
			// TODO
		}
		// other interactions
		KeyEvent {
			code: KeyCode::Enter, ..
		} => {
			// TODO
		}
		KeyEvent { code: Char('h'), .. } => app.popup = Some(make_help_text()),
		KeyEvent {
			code: Char('q') | KeyCode::Esc,
			..
		} => {
			return Ok(false);
		}
		_ => {} // ignored
	};
	Ok(true)
}

fn scroll(app: &mut App, term_size: &Size, amount: i16) {
	// TODO
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

	let mut height = 0;
	let commit_info_to_item = |ci: &CommitInfo| -> ListItem {
		let mut commit_id = ci.commit_id.to_string();
		commit_id.truncate(8);
		let lines = vec![
			Line::from(format!("{} {}", commit_id, ci.author)),
			Line::from(format!("    {}", ci.summary)),
		];
		return lines.into();
	};
	let mut commit_list_items: Vec<ListItem> = vec![];
	for commit_info in &app.commit_infos {
		commit_list_items.push(commit_info_to_item(commit_info));
		height += 1;
	}
	while height < area.height {
		let commit_info = match next_commit(app.repo, &mut app.revwalk) {
			Ok(None) => break,
			Ok(Some(ci)) => ci,
			Err(err) => {
				app.popup = Some(err.message().to_owned().into());
				break;
			}
		};
		let item = commit_info_to_item(&commit_info);
		height += 1;
		commit_list_items.push(item);
		app.commit_infos.push(commit_info);
	}
	let commit_list = List::new(commit_list_items);
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
