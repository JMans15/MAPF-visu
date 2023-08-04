#![allow(clippy::upper_case_acronyms)]

use gtk::glib::clone;
use gtk::{glib, Application, Box, FileChooserAction, FileChooserDialog, ListBox, StringList};
use gtk::{
    prelude::*, ApplicationWindow, Button, DrawingArea, DropDown, ListBoxRow, ResponseType,
    ScrolledWindow,
};
use ndarray::prelude::*;
use ndarray::Array2;
use std::cell::Cell;
use std::fs::File;
use std::io::{BufReader, Read};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use wait_timeout::ChildExt;

const APP_ID: &str = "org.gtk_rs.mapf";
const CANVAS_WIDTH: i32 = 768;
const CANVAS_HEIGHT: i32 = 768;


fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(build_ui);

    app.run()
}

fn scen_folder_parse(
    response: &ResponseType,
    scen_list: &ListBox,
    d: &FileChooserDialog,
    scen_directory: &Rc<Cell<String>>,
) {
    if *response == ResponseType::Accept {
        loop {
            let row = scen_list.row_at_index(0);
            if row.is_none() {
                break;
            }
            scen_list.remove(&row.unwrap());
        }
        let path = d.file().unwrap().path().unwrap();
        let path_clone = path;
        scen_directory.set(path_clone.to_str().unwrap().to_owned());
        let files = std::fs::read_dir(d.file().unwrap().path().unwrap()).unwrap();
        files.for_each(|file| {
            let file_name = file.unwrap().file_name();
            let row = ListBoxRow::new();
            let label = gtk::Label::new(Some(&file_name.to_string_lossy()));
            row.set_child(Some(&label));
            scen_list.append(&row);
        })
    }
    d.close();
}

fn scen_file_parse(
    scen_directory: &Rc<Cell<String>>,
    row: &ListBoxRow,
    scen_matrix: Rc<Cell<Vec<Vec<u32>>>>,
    canvas: &DrawingArea,
) {
    let dir = scen_directory.take();
    let file_path = format!(
        "{}/{}",
        dir,
        row.child()
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap()
            .label()
            .as_str()
    );
    scen_directory.set(dir);
    println!("{}", file_path);
    let scen_file = File::open(file_path).expect("Couldn't open scen file");
    let mut reader = BufReader::new(scen_file);
    let mut contents = String::new();
    let _ = reader.read_to_string(&mut contents);
    let lines = contents.lines();

    let ordered = lines
        .skip(1)
        .map(|line| {
            let split: Vec<&str> = line.split('\t').collect();
            let ints: &Vec<u32> = &split[4..8]
                .iter()
                .map(|e| -> u32 { e.parse().expect("Couldn't parse") })
                .collect();
            ints.to_owned()
        })
        .collect();
    scen_matrix.set(ordered);
    canvas.queue_draw();
}

fn result_parse() -> Array3<usize> {
    let file = std::fs::read_to_string("./.out").unwrap();
    let mut lines: Vec<&str> = file.split('\n').filter(|e| !e.is_empty()).collect();
    let header: Vec<usize> = lines
        .remove(0)
        .split(' ')
        .map(|s| s.parse().expect("Couldn't parse"))
        .collect();
    if header.len() < 2 {
        return Default::default();
    }
    // step, agent, xy
    let mut result = Array3::<usize>::default((header[1], header[0], 2));
    for step in 0..header[1] {
        for agent in 0..header[0] {
            result
                .slice_mut(s![step, agent, ..])
                .assign(&Array1::from_iter(
                    lines[step * header[0] + agent]
                        .split(',')
                        .map(|s| s.parse().expect("Couldn't parse"))
                        .collect::<Array1<usize>>(),
                ));
        }
    }
    return result;
}

fn map_file_parse(
    response: &ResponseType,
    d: &FileChooserDialog,
    grid_height: Rc<Cell<usize>>,
    grid_width: Rc<Cell<usize>>,
    map_matrix: Rc<Cell<Array2<bool>>>,
    canvas: &DrawingArea,
) {
    if *response == ResponseType::Accept {
        let file = d.file().expect("Couldn't get file");
        let filename = file.path().expect("Couldn't get path");
        let file = File::open(filename).expect("Couldn't open file");
        let mut reader = BufReader::new(file);
        let mut contents = String::new();
        let _ = reader.read_to_string(&mut contents);
        let mut lines = contents.lines().skip(1);
        grid_height.set(
            lines
                .next()
                .unwrap()
                .split(' ')
                .last()
                .unwrap()
                .parse()
                .unwrap(),
        );
        grid_width.set(
            lines
                .next()
                .unwrap()
                .split(' ')
                .last()
                .unwrap()
                .parse()
                .unwrap(),
        );

        map_matrix.set(Array2::<bool>::default((
            grid_width.get(),
            grid_height.get(),
        )));
        let mut current_matrix = map_matrix.take();
        lines.skip(1).enumerate().for_each(|(i, line)| {
            let walls = Array1::from_iter(line.chars().map(|c| c != '.'));
            current_matrix.slice_mut(s![i, ..]).assign(&walls);
        });
        map_matrix.set(current_matrix);
    }
    d.close();
    canvas.queue_draw();
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("GTK MAPF Visualizer")
        .build();

    let canvas = DrawingArea::new();
    canvas.set_size_request(CANVAS_WIDTH, CANVAS_HEIGHT);

    let button = Button::builder().label("Choose a map file").margin_top(10).build();
    let button_scen = Button::with_label("Choose scen folder");
    let button_run = gtk::Button::builder().label("Run").build();
    let nextprevbox = Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .build();
    let prevbutton = Button::builder().label("Prev").width_request(166).build();
    let nextbutton = Button::builder().label("Next").width_request(166).build();
    let allbutton = Button::builder().label("All").width_request(250).build();
    let clearbutton = Button::builder().label("Clear").width_request(250).build();
    let settings = Box::new(gtk::Orientation::Vertical, 10);
    let hbox = Box::new(gtk::Orientation::Horizontal, 10);
    let scen_list = ListBox::new();
    let scrolled_window = ScrolledWindow::new();
    scrolled_window.set_min_content_height(400);
    scrolled_window.set_min_content_width(250);
    scrolled_window.set_child(Some(&scen_list));
    let texthbox = Box::new(gtk::Orientation::Horizontal, 10);
    let nagents = gtk::Entry::builder().name("n agent").placeholder_text("# agents").build();
    let timeout = gtk::Entry::builder().name("timeout").placeholder_text("timeout [s]").build();
    

    let model = gtk::StringList::new(&["A*","A* OD","ID A*","ID A* CAT","ID CBS","ID CBS CAT","SID A*",
        "SID A* CAT","SID CBS","SID CBS CAT","CBS DS","CBS DS CAT","CBS","CBS CAT"]);
    let algo_dropdown = DropDown::new(Some(model), gtk::Expression::NONE);

    let map_matrix = Rc::new(Cell::new(Array2::<bool>::default((1, 1))));
    let scen_directory = Rc::new(Cell::new("".to_owned()));
    let scen_file = Rc::new(Cell::new("".to_owned()));
    let map_file = Rc::new(Cell::new("".to_owned()));
    let scen_matrix = Rc::new(Cell::new(Vec::<Vec<u32>>::new()));
    let grid_width = Rc::new(Cell::new(0));
    let grid_height = Rc::new(Cell::new(0));
    let sol_matrix = Rc::new(Cell::new(Array3::<usize>::default((0, 0, 0))));
    let sol_step = Rc::new(Cell::new(Option::None::<i32>));

    canvas.set_draw_func(
        clone!(@strong sol_step, @strong sol_matrix, @strong map_matrix, @strong scen_matrix, @weak grid_width, @weak grid_height, @weak nagents => move |_, cr, _, _| {
            if grid_width.get() == 0 {
                return;
            }
            let cell_w: f64 = CANVAS_WIDTH as f64 / grid_width.get() as f64;
            let cell_h: f64 = CANVAS_HEIGHT as f64 / grid_height.get() as f64;

            let solmatrix = sol_matrix.take();
            let ss = sol_step.take();
            match ss {
                None => { 
                    for step in solmatrix.axis_iter(Axis(0)) {
                        for agent in step.axis_iter(Axis(0)) {
                            cr.rectangle(agent[0] as f64 * cell_w, agent[1] as f64 * cell_h, cell_w, cell_h);
                            cr.set_source_rgb(0., 0., 1.);
                            cr.fill().unwrap();
                        }
                    }
                },
                Some(val) => {
                    for agent in solmatrix.slice(s![val, .., ..]).axis_iter(Axis(0)) {
                        cr.rectangle(agent[0] as f64 * cell_w, agent[1] as f64 * cell_h, cell_w, cell_h);
                        cr.set_source_rgb(0., 0., 1.);
                        cr.fill().unwrap();
                    }
                }
            }
            sol_step.set(ss);
            sol_matrix.set(solmatrix);

            let mmatrix = map_matrix.take();
            mmatrix.axis_iter(Axis(0)).enumerate().for_each(|(i, line)| {
                line.iter().enumerate().for_each(|(j, elem)| {
                    if *elem {
                        cr.rectangle(j as f64*cell_w, i as f64*cell_h, cell_w, cell_h);
                        cr.set_source_rgb(1.0, 1.0, 1.0);
                        cr.fill().unwrap();
                    }
                });
            });
            map_matrix.set(mmatrix);

            let smatrix = scen_matrix.take();
            let mut n = nagents.text().to_string().parse::<usize>().unwrap_or(smatrix.len());
            if smatrix.len() < n || n == 0 {
                n = smatrix.len();
            }
            smatrix.to_vec()[0..n].iter().for_each(|agent| {
                cr.rectangle(agent[0] as f64 * cell_w, agent[1] as f64 * cell_h, cell_w, cell_h);
                cr.set_source_rgb(0.0, 1.0, 0.0);
                cr.fill().unwrap();
                cr.rectangle(agent[2] as f64 * cell_w, agent[3] as f64 * cell_h, cell_w, cell_h);
                cr.set_source_rgb(1.0, 0.0, 0.0);
                cr.fill().unwrap();
            });
            scen_matrix.set(smatrix);
        }),
    );

    button_scen.connect_clicked(clone!(@weak window, @weak scen_list, @strong scen_directory => move|_| {
        let dialog = FileChooserDialog::new(
            Some("Choose a scen folder"),
            Some(&window),
            FileChooserAction::SelectFolder,
            &[("Open", ResponseType::Accept), ("Close", ResponseType::Cancel)]
        );
        dialog.connect_response(clone!(@weak scen_directory => move |d: &FileChooserDialog, response: ResponseType| {
            scen_folder_parse(&response, &scen_list, d, &scen_directory);
        }));

        dialog.show();
    }));

    // When clicking on a scenario file name
    scen_list.connect_row_activated(
        clone!(@weak sol_step, @weak sol_matrix, @weak scen_directory, @weak scen_matrix, @weak canvas, @strong scen_file => move |_, row| {
        let dir = scen_directory.take();
        let file_path = format!(
            "{}/{}",
            dir,
            row.child()
                .unwrap()
                .downcast::<gtk::Label>()
                .unwrap()
                .label()
                .as_str()
        );
            scen_directory.set(dir);
            scen_file.set(file_path);
            sol_matrix.set(Array3::<usize>::default((0, 0, 0)));
            sol_step.set(Option::None::<i32>);
            scen_file_parse(&scen_directory, row, scen_matrix, &canvas)
        }),
    );

    button.connect_clicked(clone!(@strong map_matrix, @weak window, @weak canvas, @strong map_file, @strong grid_width, @strong grid_height => move |_| {
        let dialog = FileChooserDialog::new(Some("Choose a file"),
        Some(&window), FileChooserAction::Open, &[("Open", gtk::ResponseType::Accept), ("Close", gtk::ResponseType::Cancel)]);
        dialog.connect_response(clone!(@weak map_matrix, @weak grid_width, @weak grid_height, @weak map_file => move |d: &FileChooserDialog, response: ResponseType| {
            let file = d.file();
            match file {
                None => return,
                Some(val) => map_file.set(val.path().unwrap().to_str().unwrap().to_owned()),
            }
            map_file_parse(&response, d, grid_height, grid_width, map_matrix, &canvas);
        }));
        dialog.show();
    }));

    button_run.connect_clicked(clone!(@weak timeout, @weak algo_dropdown, @weak map_file, @weak sol_matrix, @weak nagents, @weak scen_file, @weak grid_width, @weak grid_height, @weak canvas => move |_| {
        let sf = scen_file.take();
        let mf = map_file.take();
        let nthalg = algo_dropdown.selected();
        let alg = algo_dropdown.model().unwrap().downcast::<StringList>().ok().unwrap().string(nthalg).unwrap();
        println!("Running {}, {} agents, scen is {}, map is {}", alg, nagents.text().to_string(), sf, mf);
        let mut child = Command::new("./TFE_MAPF_visu")
            .arg("-a")
            .arg(alg)
            .arg("--map")
            .arg(mf.clone())
            .arg("--scen")
            .arg(sf.clone())
            .arg("-n")
            .arg(nagents.text().to_string())
        .arg("--outfile").arg("./.out").spawn().unwrap();
        let tm = timeout.text().to_string().parse::<u64>().expect("Couldn't parse");
        let status = match child.wait_timeout(Duration::from_secs(tm)).unwrap() {
            Some(st) => st.code().unwrap(),
            None => {
                child.kill().unwrap();
                child.wait().expect("oopsie");
                1
            }
        };
        println!("Ok, {}", status);
        if status == 0 {
            let res = result_parse();
            sol_matrix.set(res);
            canvas.queue_draw();
        }
        scen_file.set(sf);
        map_file.set(mf);
    }));

    nextbutton.connect_clicked(clone!(@strong sol_matrix, @weak canvas, @strong sol_step => move |_| {
        let mut ss = sol_step.take();
        let sm = sol_matrix.take();
        match ss {
            None => {
                if sm.len() <= 0 {
                    return;
                }
                ss = Some(0);
            },
            Some(val) => {
                let mut nw = val;
                if val + 1 < sm.len_of(Axis(0)) as i32 {
                    nw = val+1;
                }
                ss = Some(nw);
            },
        }
        sol_step.set(ss);
        sol_matrix.set(sm);
        canvas.queue_draw();
    }));

    prevbutton.connect_clicked(clone!(@weak canvas, @strong sol_step, @strong sol_matrix => move |_| {
        let mut ss = sol_step.take();
        let sm = sol_matrix.take();
        match ss {
            None => {
                if sm.len() <= 0 {
                    return;
                }
                let nw = sm.len_of(Axis(0)) as i32 - 1;
                ss = Some(nw);
            },
            Some(val) => {
                let nw: i32;
                if val == 0 {
                    nw = 0;
                } else {
                    nw = val - 1;
                }
                ss = Some(nw);
            },
        }
        sol_step.set(ss);
        sol_matrix.set(sm);
        canvas.queue_draw();
    }));

    allbutton.connect_clicked(clone!(@strong sol_step, @weak canvas => move |_| {
        sol_step.set(None);
        canvas.queue_draw();
    }));

    clearbutton.connect_clicked(clone!(@strong scen_list, @strong canvas, @strong map_file, @strong grid_width, @strong grid_height, @strong sol_matrix, @strong sol_step, @strong scen_file, @strong scen_directory, @strong scen_matrix, @strong map_matrix => move |_| {
        map_matrix.set(Array2::<bool>::default((1, 1)));
        scen_directory.set("".to_owned());
        scen_file.set("".to_owned());
        map_file.set("".to_owned());
        scen_matrix.set(Vec::<Vec<u32>>::new());
        grid_width.set(0);
        grid_height.set(0);
        sol_matrix.set(Array3::<usize>::default((0, 0, 0)));
        sol_step.set(Option::None::<i32>);
        loop {
            let row = scen_list.row_at_index(0);
            if row.is_none() {
                break;
            }
            scen_list.remove(&row.unwrap());
        }
        canvas.queue_draw();
    }));

    settings.append(&button);
    texthbox.append(&nagents);
    texthbox.append(&timeout);
    settings.append(&texthbox);
    settings.append(&button_scen);
    settings.append(&algo_dropdown);
    settings.append(&scrolled_window);
    settings.append(&button_run);
    settings.append(&nextprevbox);
    settings.append(&allbutton);
    settings.append(&clearbutton);
    hbox.append(&canvas);
    hbox.append(&settings);
    nextprevbox.append(&prevbutton);
    nextprevbox.append(&nextbutton);

    window.set_child(Some(&hbox));
    window.present();
}
