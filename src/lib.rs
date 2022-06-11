use ansi_term::{Color, Style};
use anyhow::{bail, Context, Result};
use chrono::{prelude::*, Duration};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::prelude::*, path::Path};

pub const DEFAULT_LIST: &str = "to-do";

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum DateMaybeTime {
    Date(NaiveDate),
    DateTime(DateTime<Local>),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TodoEntry {
    pub title: String,
    pub date: Option<DateMaybeTime>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TodoList {
    pub title: String,
    pub date: Option<DateMaybeTime>,
    list: Vec<ListItem>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ListItem {
    Entry(TodoEntry),
    List(TodoList),
}

impl DateMaybeTime {
    fn from_parts(
        date: Option<NaiveDate>,
        time: Option<NaiveTime>,
    ) -> Option<DateMaybeTime> {
        match (date, time) {
            (None, None) => None,
            (Some(date), None) => Some(DateMaybeTime::Date(date)),
            _ => {
                let datetime = match date {
                    Some(d) => d,
                    None => Local::today().naive_local(),
                }
                .and_time(time.unwrap());
                let local_time = Local.from_local_datetime(&datetime).unwrap();
                Some(DateMaybeTime::DateTime(local_time))
            }
        }
    }

    fn date_string(&self) -> String {
        let date = match self {
            DateMaybeTime::Date(date) => *date,
            DateMaybeTime::DateTime(datetime) => datetime.date().naive_local(),
        };
        date.format("%Y/%m/%d").to_string()
    }

    fn time_string(&self) -> String {
        match self {
            DateMaybeTime::Date(_) => String::new(),
            DateMaybeTime::DateTime(datetime) => {
                datetime.time().format("%H:%M").to_string()
            }
        }
    }

    fn to_string(&self) -> String {
        let (date, time) = match self {
            DateMaybeTime::Date(date) => (*date, None),
            DateMaybeTime::DateTime(datetime) => {
                (datetime.naive_local().date(), Some(datetime.time()))
            }
        };

        let today = Local::today().naive_local();
        let duration = date.signed_duration_since(today);
        let days = duration.num_days();
        let weeks = duration.num_weeks();
        let (date_string, days_till) = match days {
            -1 => (String::from("Yesterday"), String::new()),
            0 => (String::from("Today"), String::new()),
            1 => (String::from("Tomorrow"), String::new()),
            _ => {
                let (timescale, difference) = if 14 <= days.abs() {
                    (String::from("weeks"), weeks)
                } else {
                    (String::from("days"), days)
                };
                (
                    if (1..=7).contains(&days) {
                        format!("upcoming {}", date.format("%A"))
                    } else if (-7..=-1).contains(&days) {
                        format!("recent {}", date.format("%A"))
                    } else if date.year() == today.year() {
                        date.format("%B %d").to_string()
                    } else {
                        date.format("%B %d %Y").to_string()
                    },
                    if days < 0 {
                        format!("; {} {} ago", difference.abs(), timescale)
                    } else {
                        format!("; in {} {}", difference, timescale)
                    },
                )
            }
        };
        let datetime_string = match time {
            Some(t) => {
                format!("{}, {}{}", date_string, t.format("%R%P"), days_till)
            }
            None => format!("{}{}", date_string, days_till),
        };
        datetime_string
    }

    fn get_color(&self) -> Color {
        let remaining = match self {
            DateMaybeTime::Date(date) => {
                date.signed_duration_since(Local::today().naive_local())
            }
            DateMaybeTime::DateTime(datetime) => {
                datetime.signed_duration_since(Local::now())
            }
        };
        if remaining.lt(&Duration::days(0)) {
            Color::Red
        } else if remaining.lt(&Duration::days(1)) {
            Color::Yellow
        } else {
            Color::Green
        }
    }
}

impl TodoEntry {
    fn write_to(&self, out: &mut impl Write) -> std::io::Result<()> {
        if let Some(datemaybe) = self.date {
            let date_string = format!("({})", datemaybe.to_string());
            write!(
                out,
                "{} {}",
                self.title,
                datemaybe.get_color().paint(date_string)
            )
        } else {
            write!(out, "{}", self.title)
        }
    }
}

impl TodoList {
    // create default list
    pub fn default() -> TodoList {
        TodoList {
            title: String::from(DEFAULT_LIST),
            date: None,
            list: vec![ListItem::Entry(TodoEntry {
                title: String::from("Hello, world!"),
                date: Some(DateMaybeTime::DateTime(Local::now())),
            })],
        }
    }

    pub fn from_info(title: String, date: Option<DateMaybeTime>) -> TodoList {
        TodoList {
            title,
            date,
            list: Vec::new(),
        }
    }

    pub fn write_to(
        &self,
        out: &mut impl Write,
        indent: usize,
    ) -> std::io::Result<()> {
        let title = Style::new().underline().paint(self.title.as_str());
        let date_string = if let Some(datemaybe) = self.date {
            datemaybe
                .get_color()
                .paint(format!("({})", datemaybe.to_string()))
        } else {
            ansi_term::ANSIGenericString::from("")
        };
        write!(
            out,
            "{}{} {}\n",
            if indent == 0 { "   " } else { "" },
            title,
            date_string
        )
        .and(
            self.list
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let marker = match item {
                        ListItem::Entry(_) => {
                            Color::Cyan.paint(format!("{})", i))
                        }
                        ListItem::List(_) => {
                            Color::Blue.paint(format!("{}--->", i))
                        }
                    };
                    write!(out, "{}", String::from("   ").repeat(indent))
                        .and(write!(out, "{} ", marker))
                        .and(item.write_to(out, indent + 1))
                        .and(if i != self.list.len() - 1 || indent == 0 {
                            write!(out, "\n")
                        } else {
                            write!(out, "")
                        })
                })
                .collect(),
        )
    }

    pub fn write_header(&self, out: &mut impl Write) -> std::io::Result<()> {
        let title = self.title.as_str();
        let date_string = if let Some(datemaybe) = self.date {
            datemaybe
                .get_color()
                .paint(format!("({})", datemaybe.to_string()))
        } else {
            ansi_term::ANSIGenericString::from("")
        };
        write!(
            out,
            "{} {} {}\n",
            Color::Blue.paint("->"),
            title,
            date_string
        )
    }

    pub fn add_item(
        &mut self,
        item: ListItem,
        index: &mut std::slice::IterMut<'_, usize>,
    ) -> anyhow::Result<()> {
        if index.len() == 0 {
            self.list.push(item);
            Ok(())
        } else {
            let i = *index.next().unwrap();
            if i < self.list.len() {
                match self.list.get_mut(i).unwrap() {
                    ListItem::List(l) => l.add_item(item, index),
                    ListItem::Entry(_) => {
                        if index.len() == 0 {
                            if let ListItem::Entry(entry) = self.list.remove(i)
                            {
                                self.list.insert(
                                    i,
                                    ListItem::List(TodoList::from_info(
                                        entry.title,
                                        entry.date,
                                    )),
                                );
                                if let ListItem::List(new_list) =
                                    self.list.get_mut(i).unwrap()
                                {
                                    new_list.add_item(item, index)?;
                                };
                            };
                            Ok(())
                        } else {
                            bail!("Invalid index! (sub-indexing a non-list)")
                        }
                    }
                }
            } else {
                bail!("Invalid index! (too big)")
            }
        }
    }

    pub fn remove_item(
        &mut self,
        index: &mut std::slice::IterMut<'_, usize>,
    ) -> anyhow::Result<ListItem> {
        let i = *index.next().unwrap();
        if index.len() == 0 {
            if i < self.list.len() {
                Ok(self.list.remove(i))
            } else {
                bail!("Invalid index! (too big)");
            }
        } else {
            if i < self.list.len() {
                let (removed_item, empty) = match self.list.get_mut(i).unwrap()
                {
                    ListItem::List(l) => {
                        (l.remove_item(index)?, l.list.len() == 0)
                    }
                    ListItem::Entry(_) => {
                        bail!("Invalid index! (sub-indexing a non-list)");
                    }
                };
                if empty {
                    if let ListItem::List(old_list) = self.list.remove(i) {
                        self.list.insert(
                            i,
                            ListItem::Entry(TodoEntry {
                                title: old_list.title,
                                date: old_list.date,
                            }),
                        )
                    }
                };
                Ok(removed_item)
            } else {
                bail!("Invalid index! (too big)")
            }
        }
    }

    pub fn insert_item(
        &mut self,
        item: ListItem,
        index: &mut std::slice::IterMut<'_, usize>,
    ) -> anyhow::Result<()> {
        let i = *index.next().unwrap();
        if index.len() == 0 {
            if i <= self.list.len() {
                Ok(self.list.insert(i, item))
            } else {
                bail!("Invalid index! (too big)");
            }
        } else {
            if i < self.list.len() {
                match self.list.get_mut(i).unwrap() {
                    ListItem::List(l) => l.insert_item(item, index),
                    ListItem::Entry(_) => {
                        if index.len() == 1 {
                            if let ListItem::Entry(entry) = self.list.remove(i)
                            {
                                self.list.insert(
                                    i,
                                    ListItem::List(TodoList::from_info(
                                        entry.title,
                                        entry.date,
                                    )),
                                );
                                if let ListItem::List(new_list) =
                                    self.list.get_mut(i).unwrap()
                                {
                                    new_list.insert_item(item, index)?;
                                };
                            };
                            Ok(())
                        } else {
                            bail!("Invalid index! (sub-indexing a non-list)")
                        }
                    }
                }
            } else {
                bail!("Invalid index! (too big)")
            }
        }
    }

    pub fn sort(&mut self) {
        for item in self.list.iter_mut() {
            if let ListItem::List(sublist) = item {
                sublist.sort()
            }
        }
        self.list.sort_by_cached_key(|item| {
            let opt_date = match item {
                ListItem::List(l) => l.date,
                ListItem::Entry(e) => e.date,
            };
            let date_maybe = opt_date
                .unwrap_or(DateMaybeTime::Date(chrono::naive::MAX_DATE));
            match date_maybe {
                DateMaybeTime::Date(date) => (date, None),
                DateMaybeTime::DateTime(datetime) => {
                    (datetime.naive_local().date(), Some(datetime.time()))
                }
            }
        });
    }
}

impl ListItem {
    fn write_to(
        &self,
        out: &mut impl Write,
        indent: usize,
    ) -> std::io::Result<()> {
        match self {
            ListItem::Entry(entry) => entry.write_to(out),
            ListItem::List(list) => list.write_to(out, indent),
        }
    }
}

pub fn prompt_for_info(
    existing: Option<&ListItem>,
) -> Result<(String, Option<DateMaybeTime>)> {
    let mut rl = rustyline::Editor::<()>::new();
    let (prev_title, prev_date) = if let Some(listitem) = existing {
        match listitem {
            ListItem::Entry(entry) => {
                (Some(entry.title.clone()), Some(entry.date))
            }
            ListItem::List(list) => (Some(list.title.clone()), Some(list.date)),
        }
    } else {
        (None, None)
    };
    let title = loop {
        let title = match prev_title {
            Some(ref t) => rl.readline_with_initial("title: ", (&t, ""))?,
            None => rl.readline("title: ")?,
        };
        if title.len() == 0 {
            eprintln!("Please give the new list a title.",);
        } else {
            break title;
        }
    };
    let date = loop {
        let date = match prev_date {
            Some(d) => match d {
                Some(datemaybe) => rl.readline_with_initial(
                    "date (?): ",
                    (&datemaybe.date_string(), ""),
                )?,
                None => rl.readline("date (?): ")?,
            },
            None => rl.readline("date (?): ")?,
        };
        if date.len() == 0 {
            break None;
        } else {
            match NaiveDate::parse_from_str(&date, "%Y/%m/%d") {
                Ok(date) => break Some(date),
                Err(_) => eprintln!("Error parsing date (format: yyyy/mm/dd)"),
            }
        }
    };
    let time = loop {
        let time = match prev_date {
            Some(d) => match d {
                Some(datemaybe) => rl.readline_with_initial(
                    "time (?): ",
                    (&datemaybe.time_string(), ""),
                )?,
                None => rl.readline("time (?): ")?,
            },
            None => rl.readline("time (?): ")?,
        };
        if time.len() == 0 {
            break None;
        } else {
            match NaiveTime::parse_from_str(&time, "%H:%M") {
                Ok(time) => break Some(time),
                Err(_) => eprintln!("Error parsing time (format: hh:mm)"),
            }
        }
    };
    Ok((title, DateMaybeTime::from_parts(date, time)))
}

pub fn save(todo_file: &Path, lists: &HashMap<String, TodoList>) -> Result<()> {
    let json = serde_json::to_string_pretty(lists).with_context(|| {
        format!(
            "Couldn't generate to-do list file ({})",
            todo_file.display()
        )
    })?;
    std::fs::write(todo_file, json).with_context(|| {
        format!("Couldn't write to-do list file ({})", todo_file.display())
    })?;
    Ok(())
}
