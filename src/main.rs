#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;

use maud::{html, Markup, PreEscaped, Render};

fn into_markup<T>(x: T) -> Markup
where
    T: IntoIterator,
    T::Item: Render,
{
    let mut s = String::new();
    for item in x {
        item.render_to(&mut s);
    }
    PreEscaped(s)
}

fn main() -> std::io::Result<()> {
    const TRIM_TOKENS: &[char] = &['/', '*', ' ', ':', '-', '.', '^', ','];
    let mut dedup: HashMap<_, Vec<_>> = HashMap::new();

    for file in glob::glob("../rust2/**/*.rs").expect("glob pattern failed") {
        let filename = file.unwrap();
        let mut text = String::new();
        if let Err(e) = std::fs::File::open(&filename)
            .unwrap()
            .read_to_string(&mut text)
        {
            eprintln!("skipping {:?}: {}", filename, e);
            continue;
        }

        for (line_num, line) in text.lines().enumerate() {
            if !line.contains("FIXME") && !line.contains("HACK") {
                continue;
            }

            let line = line.trim_matches(TRIM_TOKENS).to_owned();
            println!("{line}");
            let filename: PathBuf = filename.iter().skip(1).collect();
            dedup
                .entry(line)
                .or_default()
                .push((filename.clone(), line_num + 1));
        }
    }
    let mut lines: Vec<_> = dedup.into_iter().collect();
    lines.sort_by(|(a, _), (b, _)| a.cmp(b));

    let fixme_regex = regex::Regex::new(r"(FIXME|HACK)\(([^\)]+)\)").unwrap();

    let doc: maud::Markup = html!(
        html {
            head {
                title {
                    "FIXMEs in the rustc source"
                }
                style {
                    "table, th, td {
                        border: 1px solid black;
                    }"
                }
            }
            body {
                table {
                    tr { th { "Description" } th { "Source" } }
                    (into_markup(lines.iter().map(|(text, entries)| {
                        let links = linkify::LinkFinder::new();
                        let mut last = 0;
                        let mut clean_text = Vec::new();
                        let bold_names = |clean_text: &mut Vec<_>, text: &str| {
                            if let Some(capture) = fixme_regex.captures(text) {
                                let found = capture.get(2).unwrap();
                                clean_text.push(html!(span {(&text[..found.start()])}));
                                clean_text.push(html!(span { strong { (found.as_str()) } }));
                                clean_text.push(html!(span { (&text[found.end()..]) }));
                            } else {
                                clean_text.push(html!(span { (text) }));
                            }
                        };
                        let issue_links = |clean_text: &mut Vec<_>, text| {
                            let mut last = 0;
                            for found in issue_references(text) {
                                if found.start != last {
                                    bold_names(clean_text, &text[last..found.start]);
                                }
                                last = found.end;
                                let found_str = &text[found.end..found.start];
                                clean_text.push(html!(span { a href=(format!("https://github.com/rust-lang/rust/issues/{}", found_str)) { (found_str) } }));
                            }
                            if last != text.len() {
                                bold_names(clean_text, &text[last..]);
                            }
                        };
                        for link in links.links(text) {
                            // fill in intermediate text
                            if link.start() != last {
                                issue_links(&mut clean_text, &text[last..link.start()]);
                            }
                            last = link.end();
                            let link_text = link.as_str().trim_start_matches("https://").trim_start_matches("github.com/");
                            clean_text.push(html!( span { a href=(link.as_str()) { (link_text) } } ));
                        }
                        if last != text.len() {
                            issue_links(&mut clean_text, &text[last..]);
                        }
                        html!(
                            tr {
                                td {
                                    (into_markup(clean_text))
                                }
                                td {
                                    (into_markup(entries.iter().map(|(file, line)| html!(
                                        a href=(format!("https://github.com/rust-lang/rust/blob/master/{}#L{}", file.display(), line)) {
                                            ({
                                                let mut file: PathBuf = file.iter().skip(1).collect();
                                                file.set_extension("");
                                                let file = file.display().to_string();
                                                let file = file.trim_start_matches("lib");
                                                file.to_owned()
                                            })
                                        }
                                        br;
                                    ))))
                                }
                            }
                        )
                    })))
                }
                script {
                    (PreEscaped(
                    "
                    // Copied Verbatim from https://stackoverflow.com/a/49041392.
                    const getCellValue = (tr, idx) => tr.children[idx].innerText || tr.children[idx].textContent;
    
                    const comparer = (idx, asc) => (a, b) => ((v1, v2) => 
                        v1 !== '' && v2 !== '' && !isNaN(v1) && !isNaN(v2) ? v1 - v2 : v1.toString().localeCompare(v2)
                        )(getCellValue(asc ? a : b, idx), getCellValue(asc ? b : a, idx));
    
                    // do the work...
                    document.querySelectorAll('th').forEach(th => th.addEventListener('click', (() => {
                        const table = th.closest('table');
                        Array.from(table.querySelectorAll('tr:nth-child(n+2)'))
                            .sort(comparer(Array.from(th.parentNode.children).indexOf(th), this.asc = !this.asc))
                            .forEach(tr => table.appendChild(tr) );
                    })));
                    "))
                }
            }
        }
    );
    let doc_str = doc.into_string();

    let _ = std::fs::create_dir("build");
    let _ = std::fs::remove_file("build/index.html");
    let mut outfile = std::fs::File::create("build/index.html")?;
    outfile.write_all(doc_str.as_bytes())
}

#[derive(Debug, PartialEq, Eq)]
struct IssueReference {
    start: usize,
    end: usize,
}

fn issue_references(text: &str) -> Vec<IssueReference> {
    // sorry, ignoring single and double digit issues
    // We can't depend on a starting `#` either, because some people just use `FIXME 1232`
    let issue_regex = regex::Regex::new(r"[^a-zA-Z]([1-9][0-9]{2,})").unwrap();

    issue_regex
        .captures_iter(text)
        .map(|m| IssueReference {
            start: m.get(1).unwrap().start(),
            end: m.get(1).unwrap().end(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_references() {
        let cases = [
            (
                "FIXME: #7698, false positive of the internal lints",
                //       ^   ^
                //       |   |
                // index 8   |
                //     index 12
                vec![IssueReference {start: 8, end: 12}],
            ),
            (
                "FIXME: 91167",
                vec![IssueReference {start:7, end: 12}]
            ),
            (
                "#[allow(dead_code)] // FIXME(81658): should be used + lint reinstated after #83171 relands",
                vec![IssueReference {start:29, end: 34}, IssueReference {start:77, end: 82}]
            ),
            (
                "ignore-android: FIXME (#20004)",
                vec![IssueReference {start:24, end: 39}]
            ),
            (
                "ignore-android: FIXME(#10381)",
                vec![IssueReference {start:23, end: 38}]
            ),

            "FIXME(jackh726): This is a hack. It's somewhat like"
        ];

        for case in cases {
            let text = case.0;
            let expected = case.1;

            assert_eq!(issue_references(text), *expected, "{text}");
        }
    }
}
