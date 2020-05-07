#[macro_use]
extern crate clap;

use std::fs;

use plotters::prelude::*;

fn main() {
    let matches = clap_app!(myapp =>
     (version: "0.1")
     (author: "Mathieu Poumeyrol <kali@zoy.org>")
     (about: "Readings library plotter")
     (@arg INPUT: +required "Sets the input file to plot")
     (@arg SINGLE_CORE: --("single-core") "Show CPU assuming single thread")
     (@arg FROM: -f --("from") +takes_value "Timestamp (seconds) or event label to start from")
     (@arg TO: -t --("to") +takes_value "Timestamp (secodns) or event label to stop at.")
    )
    .get_matches();
    plot(matches.value_of("INPUT").unwrap(), &matches).unwrap();
}

fn val<T: std::fmt::Debug + std::str::FromStr>(line: &str, ith: usize) -> T
where
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    let s = line.split_whitespace().nth(ith).unwrap();
    s.parse().unwrap()
}

fn line<'a, T: std::fmt::Debug + std::str::FromStr>(
    data: &'a [&'a str],
    ith: usize,
) -> impl Iterator<Item = (f32, T)> + 'a
where
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    data.into_iter()
        .map(move |line| (val(line, 0), val(line, ith)))
}

fn in_time_range(l: &str, start: f32, end: f32) -> bool {
    let time = val(l, 0);
    start <= time && time <= end
}

fn tranlate_time_expr(time: &str, lines: &[&str]) -> Result<f32, Box<dyn std::error::Error>> {
    time.parse::<f32>().or_else(|_| {
        lines
            .iter()
            .find(|l| l.split(" ").last().unwrap() == time)
            .map(|l| val(l, 0))
            .ok_or_else(|| format!("label {} not fond", time).into())
    })
}

fn plot(data: &str, matches: &clap::ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let png = format!("{}.png", data);
    let data = fs::read(data)?;
    assert!(data.starts_with(b"#ReadingsV1\n"));
    let data = String::from_utf8(data)?;
    let header = data.lines().nth(1).unwrap();
    let data: Vec<&str> = data.lines().skip(2).collect();

    let mut user_defined = header.split_whitespace().skip(11).collect::<Vec<_>>();
    user_defined.pop();
    let user_defined: Vec<(&str, f64)> = user_defined
        .into_iter()
        .enumerate()
        .map(|(ix, name)| {
            let max = line::<f64>(&*data, ix + 11)
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap()
                .1;
            (name, max)
        })
        .collect();
    let user_defined_len = user_defined.len();

    let time_start: f32 = tranlate_time_expr(matches.value_of("FROM").unwrap_or("0.0"), &data)?;
    let time_end: f32 = matches
        .value_of("TO")
        .map(|v| tranlate_time_expr(v, &data))
        .transpose()?
        .unwrap_or_else(|| {
            data.last()
                .unwrap()
                .split_whitespace()
                .nth(0)
                .unwrap()
                .parse()
                .unwrap()
        });

    let root = BitMapBackend::new(&*png, (1024, 768)).into_drawing_area();
    root.fill(&WHITE)?;

    let max_memory = data.iter().map(|l| val::<i64>(l, 3)).max().unwrap();
    let max_memory_range = 10i64.pow((max_memory as f64).log10() as u32 + 1);
    let mem_magnitude = ((max_memory_range as f64).log10() as usize - 2) / 3 * 3;
    let mem_magnitude_div = 10i64.pow(mem_magnitude as u32);
    let mem_magnitude_suffix = ["", "kB", "MB", "GB", "TB"][mem_magnitude / 3];

    let events = data
        .iter()
        .filter(|l| in_time_range(l, time_start, time_end))
        .filter(|l| {
            l.split_whitespace()
                .nth(11 + user_defined_len)
                .unwrap_or(&"")
                .len()
                > 0
        })
        .collect::<Vec<_>>();

    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(250)
        .y_label_area_size(50)
        .right_y_label_area_size(60)
        .margin(5)
        .build_ranged(time_start..time_end, 0f32..1.01f32)?
        .set_secondary_coord(time_start..time_end, 0..max_memory_range);

    chart
        .configure_mesh()
        .disable_x_mesh()
        .disable_y_mesh()
        .y_label_formatter(&|x| format!("{}%", (*x * 100.0) as usize))
        .draw()?;

    chart
        .configure_secondary_axes()
        .y_label_formatter(&|&x| format!("{}{}", x / mem_magnitude_div, mem_magnitude_suffix))
        .draw()?;

    for i in 0..events.len() / 2 {
        let ev_start = val(events[2 * i], 0);
        let ev_end = val(events[2 * i + 1], 0);
        chart.plotting_area().draw(&Rectangle::new(
            [(ev_start, 0.0), (ev_end, 1.0)],
            RGBColor(200, 200, 200).to_rgba().filled(),
        ))?;
    }

    let mut last_x_plotted = 0;
    for i in 0..events.len() {
        use plotters::style::text_anchor::{HPos, Pos, VPos};
        let coords = chart
            .as_coord_spec()
            .translate(&(val::<f32>(events[i], 0), -0.1));
        if i != 0 && coords.0 - last_x_plotted < 10 {
            continue;
        }
        last_x_plotted = coords.0;
        let pos = Pos::new(HPos::Right, VPos::Center);
        let style = TextStyle::from(("sans-serif", 12).into_font())
            .transform(FontTransform::Rotate270)
            .pos(pos);
        root.draw_text(events[i].split_whitespace().last().unwrap(), &style, coords)?;
    }

    let hearbeat_series = data
        .iter()
        .filter(|l| in_time_range(l, time_start, time_end))
        .filter(|l| {
            ["", "spawned_heartbeat"].contains(
                &l.split_whitespace()
                    .nth(11 + user_defined_len)
                    .unwrap_or(""),
            )
        })
        .collect::<Vec<_>>();
    let hearbeat: f32 = val::<f32>(hearbeat_series[1], 0) - val::<f32>(hearbeat_series[0], 0);
    let cores: usize = if matches.is_present("SINGLE_CORE") {
        1
    } else {
        val(data[0], 1)
    };

    let smooth_cpu = ((0.2 / hearbeat) as usize).max(1);

    let cpu_series = hearbeat_series
        .iter()
        .map(|l| val::<f32>(l, 5) + val::<f32>(l, 6))
        .collect::<Vec<_>>();
    let cpu_series = cpu_series
        .iter()
        .zip(
            std::iter::repeat(&0.0)
                .take(smooth_cpu)
                .chain(cpu_series.iter()),
        )
        .enumerate()
        .map(|(t, (b, a))| {
            (
                t as f32 * hearbeat,
                (b - a) / cores as f32 / hearbeat / smooth_cpu as f32,
            )
        })
        .collect::<Vec<_>>();

    chart.draw_series(AreaSeries::new(cpu_series, 0.0, &RED.mix(0.15)).border_style(&RED))?;

    chart.draw_secondary_series(
        AreaSeries::new(
            data.iter()
                .map(|l| (val(l, 0), val::<i64>(l, 9) - val::<i64>(l, 10))),
            0,
            &BLUE.mix(0.3),
        )
        .border_style(&BLUE),
    )?;

    chart.draw_secondary_series(
        AreaSeries::new(line(&data, 3), 0, &BLUE.mix(0.3)).border_style(&BLUE),
    )?;

    for (ix, ud) in user_defined.iter().enumerate() {
        chart
            .draw_series(LineSeries::new(
                data.iter()
                    .map(|l| (val(l, 0), (val::<f64>(l, 11 + ix) / ud.1) as f32)),
                &Palette100::pick(ix),
            ))?
            .label(format!("{} max:{}", ud.0, ud.1))
            .legend(move |(x, y)| {
                PathElement::new(vec![(x, y), (x + 20, y)], &Palette100::pick(ix))
            });
    }

    chart
        .configure_series_labels()
        .background_style(&RGBColor(128, 128, 128))
        .draw()?;

    Ok(())
}
