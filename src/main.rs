#![recursion_limit = "1024"]
#![feature(ptr_wrapping_offset_from)]

use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use typed_html::dom::DOMTree;
use typed_html::{html, text};

fn main() -> std::io::Result<()> {
    const TRIM_TOKENS: &[char] = &['#', '/', '*', '(', ')', ' ', ':', '-', '.', '^', ','];
    let mut dedup: HashMap<_, (Vec<_>, String)> = HashMap::new();
    let re = regex::Regex::new(r"[^\n]*(FIXME|HACK)[^\n]*").unwrap();
    for file in glob::glob("rust/**/*.rs").expect("glob pattern failed") {
        let filename = file.unwrap();
        let mut text = String::new();
        std::fs::File::open(&filename)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        for cap in re.find_iter(&text) {
            let line_num = text
                .lines()
                .enumerate()
                .find(|(_, s)| {
                    s.as_ptr().wrapping_offset_from(text.as_ptr()) > cap.start() as isize
                })
                .unwrap()
                .0;

            let line = cap
                .as_str()
                .trim_start_matches(TRIM_TOKENS)
                .trim_start_matches("FIXME")
                .trim_start_matches("HACK")
                .trim_start_matches(TRIM_TOKENS)
                .to_owned();
            // trim the leading `rust` part from the path
            let filename: PathBuf = filename.iter().skip(1).collect();
            dedup
                .entry(line)
                .or_insert_with(|| (Vec::new(), cap.as_str().to_owned()))
                .0.push((filename.clone(), line_num));
        }
    }
    let mut lines: Vec<_> = dedup.into_iter().collect();
    lines.sort_by(|(a, _), (b, _)| a.cmp(b));
    // sorry, ignoring single and double digit issues
    // We can't depend on a starting `#` either, because some people just use `FIXME 1232`
    let issue_regex = regex::Regex::new(r"[1-9][0-9]{2,}").unwrap();
    let user_regex = regex::Regex::new(r"\(([^\)])+\)").unwrap();
    let doc: DOMTree<String> = html!(
        <html>
            <head>
                <title>"FIXMEs in the rustc source"</title>
                <style>
                "table, th, td {
                    border: 1px solid black;
                }"
                </style>
            </head>
            <body>
                <table>
                <tr><th>"Description"</th><th>"User/Topic"</th><th>"Issue"</th><th>"Source"</th></tr>
                { lines.iter().map(|(text, (entries, raw))| {
                    let mut parser = rfind_url::Parser::new();
                    let url = text.chars().rev().enumerate().filter_map(|(i, c)| parser.advance(c).map(|n| (i, n))).next();
                    let (text, url) = match url {
                        Some((i, n)) => (
                            format!("{}{}", &text[..(text.len() - i - 1)], &text[text.len() - i - 1 + n as usize ..]),
                            Some(&text[text.len() - i - 1 ..][..n as usize]),
                        ),
                        None => (text.to_string(), None),
                    };
                    let clean_text = issue_regex.replace_all(&text, "");
                    let clean_text = clean_text
                        .replace("FIXME", "")
                        .replace("HACK", "")
                        .replace("issues", "")
                        .replace("Issues", "")
                        .replace("issue", "")
                        .replace("Issue", "");
                    let clean_text = clean_text
                        .trim_matches(TRIM_TOKENS);
                    html!(
                        <tr>
                        <td>
                            { text!(clean_text) }
                        </td>
                        <td>
                            { user_regex.find_iter(&raw).map(|user| html!(<span>{ text!(user.as_str().trim_matches(TRIM_TOKENS)) }<br/></span>)) }
                        </td>
                        <td>
                            {
                                let mut urls = Vec::new();
                                if let Some(url) = url {
                                    urls.push(html!(<span><a href={url.to_string()}>{ text!(url.trim_start_matches("https://").trim_start_matches("github.com/")) }</a><br/></span>));
                                }
                                for found in issue_regex.find_iter(&text) {
                                    let found = found.as_str();
                                    urls.push(html!(<span><a href= { format!("https://github.com/rust-lang/rust/issues/{}", found)}>{ text!(found) }</a><br/></span>));
                                }
                                urls
                            }
                        </td>
                        <td>
                            { entries.iter().map(|(file, line)| html!(
                                <a href={ format!("https://github.com/rust-lang/rust/blob/master/{}#L{}", file.display(), line) }>
                                {
                                    let mut file: PathBuf = file.iter().skip(1).collect();
                                    file.set_extension("");
                                    let file = file.display().to_string();
                                    let file = file.trim_start_matches("lib");
                                    text!("{}", file)
                                }<br/>
                                </a>
                            ))}
                        </td>
                        </tr>
                    )
                }) }
                </table>
            </body>
        </html>
    );
    let doc_str = doc.to_string();

    let _ = std::fs::create_dir("build");
    let _ = std::fs::remove_file("build/index.html");
    let mut outfile = std::fs::File::create("build/index.html")?;
    outfile.write_all(doc_str.as_bytes())
}
