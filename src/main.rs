use anyhow::{bail, Context};
use clap::{Arg, ArgGroup, Command};
use later::*;
use std::collections::HashMap;
use std::io::Read;

fn main() -> anyhow::Result<()> {
    let matches = Command::new("later")
        .about("Autumn's to-do list program")
        .long_about("This program allows nested lists. The index of a nested list should be given as a comma-separated list of integers starting with the top-level list index. e.g. `later add 1,3,1,2`")
        .arg(
            Arg::new("list-name")
                .help("name of to-do list")
                .takes_value(true)
                .value_name("LIST NAME"),
        )
        .subcommands(vec![
            Command::new("add")
                .short_flag('a')
                .about("add to a list")
                .arg(
                    Arg::new("index")
                        .help("index to insert sub-item")
                        .takes_value(true)
                        .forbid_empty_values(true)
                        .use_value_delimiter(true)
                        .require_value_delimiter(true),
                )
                .arg(
                    Arg::new("name")
                        .help("name of item to add")
                        .takes_value(true),
                ),
            Command::new("remove")
                .short_flag('r')
                .about("remove from a list")
                .arg(
                    Arg::new("index")
                        .help("index of item to remove")
                        .required(true)
                        .takes_value(true)
                        .forbid_empty_values(true)
                        .use_value_delimiter(true)
                        .require_value_delimiter(true),
                ),
            Command::new("list")
                .short_flag('l')
                .about("interact with the list of lists")
                .args(vec![
                    Arg::new("add")
                        .short('a')
                        .long("add")
                        .help("create a new to-do list")
                        .takes_value(true)
                        .value_name("LIST NAME")
                        .min_values(0)
                        .multiple_values(false),
                    Arg::new("remove")
                        .short('r')
                        .long("remove")
                        .help("delete a to-do list")
                        .takes_value(true)
                        .value_name("LIST NAME"),
                    Arg::new("edit")
                        .short('e')
                        .long("edit")
                        .help("edit a to-do list")
                        .takes_value(true)
                        .value_name("LIST NAME"),
                ])
                .group(
                    ArgGroup::new("list_funcs")
                        .args(&["add", "remove", "edit"]),
                ),
            Command::new("move")
                .short_flag('m')
                .about("move item in a list")
                .arg(
                    Arg::new("from")
                        .help("index of item to move")
                        .required(true)
                        .takes_value(true)
                        .use_value_delimiter(true)
                        .require_value_delimiter(true),
                )
                .arg(
                    Arg::new("to")
                        .help("index at which to insert item")
                        .required(true)
                        .takes_value(true)
                        .use_value_delimiter(true)
                        .require_value_delimiter(true),
                ),
            Command::new("edit")
                .short_flag('e')
                .about("edit item in a list")
                .arg(
                    Arg::new("index")
                        .help("index of item to edit")
                        .required(true)
                        .use_value_delimiter(true)
                        .require_value_delimiter(true),
                ),
        ])
        .get_matches();

    let mut todo_file = if let Some(mut path) = dirs::data_local_dir() {
        path.push("later");
        path
    } else {
        bail!("Could not find standard local data directory.")
    };
    std::fs::DirBuilder::new()
        .recursive(true)
        .create(todo_file.clone())?;
    todo_file.push("later.json");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .read(true)
        .create(true)
        .open(&todo_file)?;
    let mut s = String::new();
    file.read_to_string(&mut s).with_context(|| {
        format!("Couldn't read to-do list file ({})", &todo_file.display())
    })?;
    let mut lists: HashMap<String, TodoList> = if s.is_empty() {
        let mut m = HashMap::new();
        m.insert(String::from(DEFAULT_LIST), TodoList::default());
        println!("Generating new storage file in {}", todo_file.display());
        save(&todo_file, &m)?;
        m
    } else {
        serde_json::from_str(&s).with_context(|| {
            format!("Couldn't parse to-do list file ({})", todo_file.display())
        })?
    };
    let list_name = if matches.is_present("list-name") {
        matches.value_of("list-name").unwrap()
    } else {
        if !lists.contains_key(DEFAULT_LIST) {
            lists.insert(String::from(DEFAULT_LIST), TodoList::default());
        }
        DEFAULT_LIST
    };
    let active_list =
        if let Some(list) = lists.get_mut(&String::from(list_name)) {
            list
        } else {
            bail!("List name not found!");
        };

    let mut stdout = std::io::stdout();
    match matches.subcommand() {
        Some(("list", list_matches)) => {
            if list_matches.is_present("add") {
                let (title, date) =
                    match list_matches.value_of_t::<String>("add") {
                        Ok(title) => (title, None),
                        Err(_) => prompt_for_info(None)?,
                    };
                if lists.contains_key(&title) {
                    bail!("The list '{}' already exists", title);
                }
                lists.insert(
                    title.clone(),
                    TodoList::from_info(title.clone(), date),
                );
                save(&todo_file, &lists)?;
                println!("added new to-do list: '{}'", title);
            } else if list_matches.is_present("remove") {
                let title: String = list_matches.value_of_t_or_exit("remove");
                if !lists.contains_key(&title) {
                    bail!(
                        "The to-do list '{}' does not currently exist",
                        title
                    );
                } else if title == DEFAULT_LIST {
                    bail!("You cannot remove the default to-do list!");
                }
                let mut rl = rustyline::Editor::<()>::new();
                let confirm =
                    rl.readline(&format!("Remove list '{}'? (y/N): ", title))?;
                if confirm.to_lowercase() == "y" {
                    lists.remove(&title);
                    save(&todo_file, &lists)?;
                    println!("removed to-do list: '{}'", title);
                } else {
                    bail!("Cancelled.");
                }
            } else if list_matches.is_present("edit") {
                let title: String = list_matches.value_of_t_or_exit("edit");
                if !lists.contains_key(&title) {
                    bail!("The list '{}' does not currently exist", title);
                }
                let removed_list = lists.remove(&title).unwrap();
                let list_item = ListItem::List(removed_list);
                let (new_title, new_date) = prompt_for_info(Some(&list_item))?;
                if let ListItem::List(mut l) = list_item {
                    l.title = new_title.clone();
                    l.date = new_date;
                    if lists.contains_key(&new_title) {
                        bail!(
                            "The list '{}' already exists. Edit reverted.",
                            new_title
                        );
                    }
                    lists.insert(new_title.clone(), l);
                    save(&todo_file, &lists)?;
                }
            }
            if lists.len() == 1 {
                eprintln!("No named lists exist currently. (Use `later list --add` to create one.)");
            } else {
                let mut v: Vec<(&String, &TodoList)> = lists.iter().collect();
                v.sort_by_key(|(title, _)| *title);
                v.iter()
                    .filter(|(title, _)| *title != DEFAULT_LIST)
                    .map(|(_, list)| list.write_header(&mut stdout))
                    .for_each(drop);
            }
            return Ok(());
        }
        Some(("add", add_matches)) => {
            let (name, mut index) = match (
                add_matches.is_present("name"),
                add_matches.is_present("index"),
            ) {
                (true, true) => {
                    let name: String = add_matches.value_of_t_or_exit("name");
                    let index: Vec<usize> =
                        add_matches.values_of_t_or_exit("index");
                    (Some(name), index)
                }
                (false, true) => {
                    let index: Result<Vec<usize>, _> =
                        add_matches.values_of_t("index");
                    if let Ok(v) = index {
                        (None, v)
                    } else {
                        let name_pieces: Vec<String> =
                            add_matches.values_of_t_or_exit("index");
                        let name = name_pieces
                            .into_iter()
                            .reduce(|acc, item| acc + "," + &item)
                            .unwrap();
                        (Some(name), Vec::new())
                    }
                }
                _ => (None, Vec::new()),
            };
            let (title, date) = match name {
                Some(s) => (s, None),
                None => prompt_for_info(None)?,
            };
            active_list.add_item(
                ListItem::Entry(TodoEntry { title, date }),
                &mut index.iter_mut(),
            )?;
            save(&todo_file, &lists)?;
        }
        Some(("remove", remove_matches)) => {
            let mut index: Vec<usize> =
                remove_matches.values_of_t_or_exit("index");
            let mut rl = rustyline::Editor::<()>::new();
            if match active_list.remove_item(&mut index.iter_mut())? {
                ListItem::List(l) => {
                    let confirm = rl.readline(&format!(
                        "Remove sublist '{}'? (y/N): ",
                        l.title
                    ))?;
                    confirm.to_lowercase() == "y"
                }
                ListItem::Entry(e) => {
                    let confirm = rl.readline(&format!(
                        "Remove entry '{}'? (Y/n): ",
                        e.title
                    ))?;
                    confirm.to_lowercase() == "y" || confirm == ""
                }
            } {
                save(&todo_file, &lists)?;
            } else {
                bail!("Cancelled.");
            }
        }
        Some(("move", move_matches)) => {
            let mut from_index: Vec<usize> =
                move_matches.values_of_t_or_exit("from");
            let mut to_index: Vec<usize> =
                move_matches.values_of_t_or_exit("to");
            let item = active_list.remove_item(&mut from_index.iter_mut())?;
            active_list.insert_item(item, &mut to_index.iter_mut())?;
            save(&todo_file, &lists)?;
        }
        Some(("edit", edit_matches)) => {
            let mut index: Vec<usize> =
                edit_matches.values_of_t_or_exit("index");
            let item = active_list.remove_item(&mut index.iter_mut())?;
            let (new_title, new_date) = prompt_for_info(Some(&item))?;
            match item {
                ListItem::Entry(mut entry) => {
                    entry.title = new_title;
                    entry.date = new_date;
                    active_list.insert_item(
                        ListItem::Entry(entry),
                        &mut index.iter_mut(),
                    )?;
                }
                ListItem::List(mut list) => {
                    list.title = new_title;
                    list.date = new_date;
                    active_list.insert_item(
                        ListItem::List(list),
                        &mut index.iter_mut(),
                    )?;
                }
            }
            save(&todo_file, &lists)?;
        }
        _ => {}
    }
    let active_list = lists.get_mut(&String::from(list_name)).unwrap();
    active_list.write_to(&mut stdout, 0)?;
    Ok(())
}
