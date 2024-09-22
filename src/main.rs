use std::{collections::HashMap, io::{BufRead, BufReader, BufWriter, Read, Write}};

use anyhow::{bail, Error};
use argh::FromArgs;

const ENVVAR_DEFAULT_FORMAT: &str = "CSV2JSON_DEFAULT_FORMAT";
const ENVVAR_INITIAL_VECTOR_CAPACITY: &str = "CSV2JSON_INITIAL_VECTOR_CAPACITY";


const FORMAT_NAME_LIST_OF_MAPS: &str = "list-of-maps";
const FORMAT_NAME_LIST_OF_MAPS_SHORTENED: &str = "lom";
const FORMAT_NAME_MAP_OF_LISTS: &str = "map-of-lists";
const FORMAT_NAME_MAP_OF_LISTS_SHORTENED: &str = "mol";

const FORMAT_DEFAULT: &str = FORMAT_NAME_MAP_OF_LISTS;

#[derive(Clone)]
enum CsvResult {
    MapOfLists(HashMap<String, Vec<String>>),
    ListOfMaps(Vec<HashMap<String, String>>)
}

impl CsvResult {
    fn from_format_str(s: &str) -> Result<Self, Error> {
        let map = CsvResult::_get_str_map();
        if let Some(format) = map.get(s) {
            Ok(format.clone())
        }
        else {
            bail!("Unknown format string: {}", s)
        }
    }

    fn _get_str_map() -> HashMap<&'static str, CsvResult> {
        let mut map = HashMap::new();
        map.insert(FORMAT_NAME_LIST_OF_MAPS, CsvResult::ListOfMaps(Vec::new()));
        map.insert(FORMAT_NAME_LIST_OF_MAPS_SHORTENED, CsvResult::ListOfMaps(Vec::new()));
        map.insert(&FORMAT_NAME_LIST_OF_MAPS[0..1], CsvResult::ListOfMaps(Vec::new()));

        map.insert(FORMAT_NAME_MAP_OF_LISTS, CsvResult::MapOfLists(HashMap::new()));
        map.insert(FORMAT_NAME_MAP_OF_LISTS_SHORTENED, CsvResult::MapOfLists(HashMap::new()));
        map.insert(&FORMAT_NAME_MAP_OF_LISTS[0..1], CsvResult::MapOfLists(HashMap::new()));

        map
    }
}

#[derive(FromArgs)]
/// CSV in; JSON out
struct Args {
    /// JSON format; either "list-of-maps" or "map-of-lists"
    #[argh(option, short = 'f', long = "format", default = "default_format()")]
    format: String,

    /// output file; omit to output to STDOUT
    #[argh(option, short = 'o', long = "out")]
    output_file: Option<String>,

    /// input file; omit to input from STDIN
    #[argh(option, short = 'i', long = "in")]
    input_file: Option<String>,

    /// allows anomalies in CSV format
    #[argh(switch, short = 'a', long = "allow-anomalies")]
    allow_anomalies: bool,

    /// pretty json output
    #[argh(switch, short = 'p', long = "pretty")]
    pretty: bool,
}

fn main() -> Result<(), Error> {
    let args: Args = argh::from_env();
    open_output(args.output_file)?
        .write_all(
        process_input(
                open_input(args.input_file)?,
                CsvResult::from_format_str(&args.format)?,
                args.allow_anomalies,
                args.pretty)?.as_bytes()
            )?;
    Ok(())
}

fn process_input(
    mut input: BufReader<Box<dyn Read>>,
    mut result: CsvResult,
    allow_anomalies: bool,
    pretty: bool
) -> Result<String, Error> {
    let mut buffer = String::new();
    let mut headers = None;
    while let Some(line) = read_line(&mut input, &mut buffer) {
        let tokens = line?.split(',');
        if let Some(header_map) = headers.as_mut() {
            match &mut result {
                CsvResult::MapOfLists(map_of_lists) => {
                    process_line_for_map_of_lists(tokens, header_map, map_of_lists, allow_anomalies)?;
                },
                CsvResult::ListOfMaps(list_of_maps) => {
                    process_line_for_list_of_maps(tokens, header_map, list_of_maps, allow_anomalies)?;
                },
            }
        }
        else {
            headers = Some(process_headers(tokens));
        }
    }

    to_json_str(&result, pretty)
}

fn to_json_str(result: &CsvResult, pretty: bool) -> Result<String, Error> {
    let json_str;
    if pretty {
        json_str = format!("{}\n", match result {
            CsvResult::MapOfLists(map_of_lists) => serde_json::to_string_pretty(map_of_lists)?,
            CsvResult::ListOfMaps(list_of_maps) => serde_json::to_string_pretty(list_of_maps)?,
        });
    }
    else {
        json_str = match result {
            CsvResult::MapOfLists(map_of_lists) => serde_json::to_string(map_of_lists)?,
            CsvResult::ListOfMaps(list_of_maps) => serde_json::to_string(list_of_maps)?,
        }
    }

    Ok(json_str)
}

fn process_headers<'a>(tokens: impl Iterator<Item = &'a str>) -> HashMap<usize, String> {
    let mut i: usize = 0;
    let mut map = HashMap::new();
    for token in tokens {
        map.insert(i, token.replace("\n", ""));
        i += 1;
    }

    map
}

fn process_line_for_map_of_lists<'a>(
    tokens: impl Iterator<Item = &'a str>,
    headers: &HashMap<usize, String>,
    map_of_lists: &mut HashMap<String, Vec<String>>,
    allow_anomalies: bool
) -> Result<(), Error> {
    let mut i: usize = 0;
    for token in tokens {
        if let Some(column_name) = headers.get(&i) {
            if map_of_lists.get(column_name).is_none() {
                map_of_lists.insert(column_name.clone(), Vec::with_capacity(get_initial_vec_capacity()));
            }

            let column: &mut Vec<String> = map_of_lists.get_mut(column_name).unwrap().as_mut();
            column.push(token.replace("\n", ""));
        }
        else {
            let msg = format!("Found item outside of expected bounds; index: {}", i);
            if allow_anomalies {
                eprintln!("{}", msg);
            }
            else {
                bail!(msg)
            }
        }

        i += 1;
    }

    if i < headers.len() {
        let msg = format!("Line too short; length: {}; expected: {}", headers.len(), i);
        if allow_anomalies {
            eprintln!("Warning: {}", msg);
        }
        else {
            bail!(msg)
        }
    }

    Ok(())
}

fn process_line_for_list_of_maps<'a>(
    tokens: impl Iterator<Item = &'a str>,
    headers: &HashMap<usize, String>,
    list_of_maps: &mut Vec<HashMap<String, String>>,
    allow_anomalies: bool
) -> Result<(), Error> {
    let mut map = HashMap::new();
    let mut i: usize = 0;
    for token in tokens {
        if let Some(column_name) = headers.get(&i) {
            map.insert(column_name.clone(), token.replace("\n", ""));
        }
        else {
            let msg = format!("Found item outside of expected bounds; index: {}", i);
            if allow_anomalies {
                eprintln!("Warning: {}", msg);
            }
            else {
                bail!(msg)
            }
        }
        i += 1;
    }

    if i < headers.len() {
        let msg = format!("Line too short; length: {}; expected: {}", headers.len(), i);
        if allow_anomalies {
            eprintln!("Warning: {}", msg);
        }
        else {
            bail!(msg)
        }
    }

    list_of_maps.push(map);

    Ok(())
}

fn read_line<'buf, T>(
    reader: &mut BufReader<T>,
    buffer: &'buf mut String,
) -> Option<std::io::Result<&'buf mut String>> 
where 
    T: ?Sized,
    T: Read
{
    buffer.clear();

    reader
        .read_line(buffer)
        .map(|u| if u == 0 { None } else { Some(buffer) })
        .transpose()
}

fn open_input(input_file: Option<String>) -> Result<BufReader<Box<dyn Read>>, Error> {
    let input: Box<dyn Read + 'static>;
    if let Some(input_file) = input_file {
        input = Box::new(std::fs::File::open(input_file)?);
    }
    else {
        input = Box::new(std::io::stdin());
    }

    Ok(BufReader::new(input))
}

fn open_output(output_file: Option<String>) -> Result<BufWriter<Box<dyn Write>>, Error> {
    let output: Box<dyn Write + 'static>;
    if let Some(output_file) = output_file {
        output = Box::new(std::fs::File::create(output_file)?);
    }
    else {
        output = Box::new(std::io::stdout());
    }

    Ok(BufWriter::new(output))
}

fn default_format() -> String {
    if let Ok(val) = std::env::var(ENVVAR_DEFAULT_FORMAT) {
        val
    }
    else {
        FORMAT_DEFAULT.to_string()
    }
}

fn get_initial_vec_capacity() -> usize {
    const DEFAULT: usize = 1024;
    if let Ok(val) = std::env::var(ENVVAR_INITIAL_VECTOR_CAPACITY) {
        if let Ok(result) = str::parse::<usize>(&val) {
            return result
        }
        else {
            eprintln!("Warning: {} value of {} could not be parsed as integer; defaulting to {}",
                ENVVAR_INITIAL_VECTOR_CAPACITY, val, DEFAULT);
        }
    }
    
    DEFAULT
}
