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

use crate::git::{next_commit, show, CommitInfo};

pub struct App<'repo> {
	repo: &'repo Repository,
	commit_infos: Vec<CommitInfo<'repo>>,
	revwalk: Revwalk<'repo>,
	log_mode: LogMode,
	log_state: ListState,
	show_commit: Option<CommitView>,
	popup: Option<Text<'static>>,
}

struct CommitView {
	index: usize,
	files_state: ListState,
	file_view: Option<FileView>,
}

struct FileView {
	contents: Text<'static>,
	scroll: u16,
}

impl App<'_> {
	pub fn new<'a>(repo: &'a Repository, revwalk: Revwalk<'a>) -> App<'a> {
		App {
			repo,
			commit_infos: vec![],
			revwalk,
			log_mode: LogMode::Short,
			log_state: ListState::default(),
			show_commit: None,
			popup: None,
		}
	}

	fn show_commit_file(&mut self, index: usize) {
		let show_commit = self.show_commit.as_mut().unwrap();
		show_commit.show_file(self.repo, &self.commit_infos, index);
	}
}

impl CommitView {
	fn show_file(&mut self, repo: &Repository, commit_infos: &[CommitInfo], index: usize) {
		let commit = &commit_infos[self.index];
		if let Some(path) = commit.patch.get_delta(index).unwrap().new_file().path() {
			self.file_view = Some(FileView {
				contents: show(repo, commit.commit_id, path),
				scroll: 0,
			});
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

	if let Some(ref mut show_commit) = app.show_commit {
		match key {
			KeyEvent {
				code: Char('j') | KeyCode::Down,
				..
			} => scroll_file(&mut show_commit.file_view, term_size, 1),
			KeyEvent {
				code: Char('k') | KeyCode::Up,
				..
			} => scroll_file(&mut show_commit.file_view, term_size, -1),
			KeyEvent {
				code: Char('n'),
				..
			} => scroll(&mut show_commit.files_state, 1),
			KeyEvent {
				code: Char('p'),
				..
			} => scroll(&mut show_commit.files_state, -1),
			KeyEvent {
				code: KeyCode::Enter, ..
			} => {
				if let Some(index) = show_commit.files_state.selected() {
					app.show_commit_file(index);
				}
			},
			KeyEvent {
				code: Char('q') | KeyCode::Esc,
				..
			} => {
				app.show_commit = None;
			},
			_ => {}, // ignored
		}
		return Ok(true);
	}

	match key {
		// scroll
		KeyEvent {
			code: Char('j') | KeyCode::Down,
			..
		} => scroll(&mut app.log_state, 1),
		KeyEvent {
			code: Char('k') | KeyCode::Up,
			..
		} => scroll(&mut app.log_state, -1),
		KeyEvent { code: Char('d'), .. }
		| KeyEvent {
			code: KeyCode::PageDown,
			..
		} => scroll(&mut app.log_state, (term_size.height / 2).try_into().unwrap()),
		KeyEvent { code: Char('u'), .. }
		| KeyEvent {
			code: KeyCode::PageUp, ..
		} => scroll(&mut app.log_state, -i16::try_from(term_size.height / 2).unwrap()),
		KeyEvent { code: Char('g'), .. }
		| KeyEvent {
			code: KeyCode::Home, ..
		} => {
			app.log_state.select_first();
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
		KeyEvent {
			code: KeyCode::Enter, ..
		} => {
			if let Some(index) = app.log_state.selected() {
				let mut files_state = ListState::default();
				let mut show_file = None;
				let commit = &app.commit_infos[index];
				if let Some(delta) = commit.patch.get_delta(0) {
					if let Some(path) = delta.new_file().path() {
						// immediately show the first file
						files_state.select_first();
						show_file = Some(FileView {
							contents: show(app.repo, commit.commit_id, path),
							scroll: 0,
						});
					}
				}

				app.show_commit = Some(CommitView {
					index,
					files_state,
					file_view: show_file,
				});
			}
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

fn scroll(list_state: &mut ListState, amount: i16) {
	match list_state.selected() {
		None => list_state.select(Some(0)),
		Some(index) => {
			let new_index = index.saturating_add_signed(amount.into());
			list_state.select(Some(new_index.max(0)));
		},
	}
}
fn scroll_file(show_file_option: &mut Option<FileView>, term_size: &Size, amount: i16) {
	if let Some(ref mut show_file) = show_file_option {
		let max = u16::try_from(show_file.contents.height()).unwrap_or(u16::MAX).saturating_sub(term_size.height / 3);
		show_file.scroll = show_file.scroll.saturating_add_signed(amount).clamp(0, max);
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

	let highlight_style = Style::default().bg(Color::Indexed(237)); // 232 is black, 255 is white; 237 is dark gray
	match app.show_commit {
		None => {
			// log view
			let commit_list = List::new(app.commit_infos.iter().map(|ci| commit_info_to_item(ci, &app.log_mode)))
				.highlight_style(highlight_style);
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
		},
		Some(ref mut show_commit) => {
			// show view
			let cap_direction = if area.width / 2 > area.height {
				Direction::Horizontal
			} else {
				Direction::Vertical
			};
			let commit_and_patch = Layout::default()
				.constraints(Constraint::from_percentages([50, 50]))
				.direction(cap_direction)
				.split(area);
			let message_and_files = Layout::default()
				.direction(Direction::Vertical)
				.constraints(Constraint::from_percentages([50, 50]))
				.split(commit_and_patch[0]);
			let commit = &app.commit_infos[show_commit.index];

			let commit_message = Paragraph::new(commit.message.clone())
				.block(Block::bordered().title(commit.commit_id.to_string()).title_style(Style::new().yellow()));
			frame.render_widget(commit_message, message_and_files[0]);

			let mut commit_file_items = vec![];
			for delta in commit.patch.deltas() {
				commit_file_items.push(match delta.new_file().path() {
					Some(file_path) => file_path.to_string_lossy(),
					None => "".into(),
				});
			}
			let commit_files = List::new(commit_file_items).highlight_style(highlight_style);
			frame.render_stateful_widget(commit_files, message_and_files[1], &mut show_commit.files_state);

			if let Some(show_file) = &mut show_commit.file_view {
				frame.render_widget(
					Paragraph::new(show_file.contents.clone())
						.wrap(Wrap { trim: false })
						.scroll((show_file.scroll, 0))
						.block(Block::bordered()),
					commit_and_patch[1],
				);
			}
		},
	}
}

fn commit_info_to_item<'a>(ci: &'a CommitInfo, log_mode: &LogMode) -> ListItem<'a> {
	let mut commit_id = ci.commit_id.to_string();
	commit_id.truncate(8);
	let mut lines = vec![Line::from(vec![
		Span::from(commit_id).yellow(),
		" ".to_span(),
		ci.time.to_span().green(),
		" ".to_span(),
		ci.author_name.to_span().light_blue().bold(),
		Span::from(format!(" <{}>", ci.author_email)).blue(),
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
