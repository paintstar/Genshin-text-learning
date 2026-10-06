//! DictSearchService — 全应用唯一的词典查询函数（架构 §3.2 / 技术设计 §4.2-4）。
//!
//! 查询范围 = dict.db（`dict_ro` 只读挂载）+ app.db 术语表（term/term_text）
//! 的应用层合并。入参为**候选形态有序列表**（词面 → 还原形 → 读音，各带来源
//! 标注）——划词链路（§3.3）与字典页（§4.3）完全同一函数。
//!
//! 合并优先级：① 自有术语表条目（徽标「术语表」，最优先）→ ② 带中文释义的
//! 词典条目（zhwiktionary 优先、JMnedict 补充）→ ③ 仅英文释义条目（JMdict
//! en 兜底）。外部源内同 (headword, reading) 合并为一条逻辑词条；术语表条目
//! 不与外部词条合并。

use rusqlite::Connection;
use shared::dto::{CandidateForm, DictEntryDto, DictSearchResult, DictSource, FormKind, GlossDto, LangText};
use shared::AppError;
use std::collections::BTreeMap;

pub struct DictSearchService;

struct RawEntry {
    entry_id: i64,
    source: String,
    headword: String,
    reading_kana: Option<String>,
    pos_json: Option<String>,
    common: bool,
}

/// 单个候选形态的查询结果（含命中标注）。
pub struct MergedEntry {
    pub dto: DictEntryDto,
}

fn parse_gloss_json(json: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(json).unwrap_or_default()
}

fn parse_pos_json(json: &Option<String>) -> Vec<String> {
    json.as_deref()
        .and_then(|j| serde_json::from_str::<Vec<String>>(j).ok())
        .unwrap_or_default()
}

impl DictSearchService {
    /// 唯一查询入口。`dict_available = false` 时返回空结果并标记词典不可用
    /// （资源缺失显式停用语义；调用方据此禁用词典 UI 而非呈现空成功）。
    pub fn search(conn: &Connection, candidates: &[CandidateForm]) -> Result<DictSearchResult, AppError> {
        if !crate::is_mounted(conn) {
            return Ok(DictSearchResult {
                entries: vec![],
                dict_available: false,
            });
        }
        let mut out: Vec<DictEntryDto> = Vec::new();

        // ① 术语表条目（最优先；游戏文本码族 jp/chs 表记对）。
        for cand in candidates {
            // 第一步：按表记命中 term_id。
            let ids: Vec<i64> = {
                let mut stmt = conn
                    .prepare("SELECT DISTINCT term_id FROM term_text WHERE text = ?1 LIMIT 20")
                    .map_err(crate::q)?;
                let rows = stmt
                    .query_map([&cand.form], |r| r.get(0))
                    .map_err(crate::q)?
                    .collect::<Result<Vec<i64>, _>>()
                    .map_err(crate::q)?;
                rows
            };
            if ids.is_empty() {
                continue;
            }
            // 第二步：取这些术语的全部跨语言表记对。
            let mut by_term: BTreeMap<i64, (String, Vec<LangText>)> = BTreeMap::new();
            {
                let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                let sql = format!(
                    "SELECT t.term_id, t.source, x.lang, x.text
                     FROM term t JOIN term_text x ON x.term_id = t.term_id
                     WHERE t.term_id IN ({placeholders}) ORDER BY t.term_id, x.lang"
                );
                let mut stmt = conn.prepare(&sql).map_err(crate::q)?;
                let rows = stmt
                    .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, String>(3)?,
                        ))
                    })
                    .map_err(crate::q)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(crate::q)?;
                for (id, source, lang, text) in rows {
                    by_term
                        .entry(id)
                        .or_insert_with(|| (source, vec![]))
                        .1
                        .push(LangText { lang, text });
                }
            }
            let found = !by_term.is_empty();
            let terms: Vec<(String, Vec<LangText>)> = by_term.into_values().collect();
            for (_source, texts) in terms {
                out.push(DictEntryDto {
                    headword: cand.form.clone(),
                    reading_kana: None,
                    pos: vec![],
                    glosses: vec![],
                    source: DictSource::Term,
                    common: true,
                    matched_form: cand.form.clone(),
                    matched_form_kind: cand.form_kind,
                    matched_source_note: cand.source_note.clone(),
                    term_texts: Some(texts),
                });
            }
            if found {
                break; // 术语表命中即止（不重复查词典同形态）。
            }
        }

        // ②③ 词典条目：按候选形态顺序查询，外部源内按 (headword, reading) 合并。
        let mut merged: BTreeMap<(String, Option<String>), DictEntryDto> = BTreeMap::new();
        for cand in candidates {
            let mut stmt = conn
                .prepare(
                    "SELECT e.entry_id, e.source, e.headword, e.reading_kana, e.pos_json, e.common
                     FROM dict_ro.dict_index i JOIN dict_ro.dict_entry e ON e.entry_id = i.entry_id
                     WHERE i.form = ?1 LIMIT 60",
                )
                .map_err(crate::q)?;
            let entries = stmt
                .query_map([&cand.form], |r| {
                    Ok(RawEntry {
                        entry_id: r.get(0)?,
                        source: r.get(1)?,
                        headword: r.get(2)?,
                        reading_kana: r.get(3)?,
                        pos_json: r.get(4)?,
                        common: r.get::<_, i64>(5)? != 0,
                    })
                })
                .map_err(crate::q)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(crate::q)?;
            for e in entries {
                // 释义按 lang 行存在性组织。
                let mut gstmt = conn
                    .prepare("SELECT lang, gloss_json FROM dict_ro.dict_gloss WHERE entry_id = ?1")
                    .map_err(crate::q)?;
                let glosses = gstmt
                    .query_map([e.entry_id], |r| {
                        Ok(GlossDto {
                            lang: r.get(0)?,
                            texts: parse_gloss_json(&r.get::<_, String>(1)?),
                        })
                    })
                    .map_err(crate::q)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(crate::q)?;
                let source = match e.source.as_str() {
                    "zhwiktionary" => DictSource::Zhwiktionary,
                    "jmnedict" => DictSource::Jmnedict,
                    _ => DictSource::Jmdict,
                };
                let key = (e.headword.clone(), e.reading_kana.clone());
                match merged.get_mut(&key) {
                    Some(existing) => {
                        // 同词条合并释义行（保留来源徽标：中文释义源优先）。
                        for g in glosses {
                            if !existing.glosses.iter().any(|x| x.lang == g.lang) {
                                existing.glosses.push(g);
                            }
                        }
                        if matches!(source, DictSource::Zhwiktionary) {
                            existing.source = DictSource::Zhwiktionary;
                        }
                        existing.common = existing.common || e.common;
                    }
                    None => {
                        merged.insert(
                            key,
                            DictEntryDto {
                                headword: e.headword.clone(),
                                reading_kana: e.reading_kana.clone(),
                                pos: parse_pos_json(&e.pos_json),
                                glosses,
                                source,
                                common: e.common,
                                matched_form: cand.form.clone(),
                                matched_form_kind: cand.form_kind,
                                matched_source_note: cand.source_note.clone(),
                                term_texts: None,
                            },
                        );
                    }
                }
            }
        }

        // 排序：带中文释义优先（zhwiktionary > jmnedict）、common 加权、仅英文兜底最后。
        let mut list: Vec<DictEntryDto> = merged.into_values().collect();
        list.sort_by(|a, b| {
            let zh = |e: &DictEntryDto| e.glosses.iter().any(|g| g.lang == "zh");
            match (zh(a), zh(b)) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => match (a.common, b.common) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.headword.cmp(&b.headword),
                },
            }
        });
        out.extend(list);
        Ok(DictSearchResult {
            entries: out,
            dict_available: true,
        })
    }
}

/// 便捷：生成划词链路的候选形态有序列表（词面 → 还原形 → 读音）。
pub fn candidates_from_token(surface: &str, base: Option<&str>, reading: Option<&str>) -> Vec<CandidateForm> {
    let mut v = vec![CandidateForm {
        form: surface.to_string(),
        form_kind: FormKind::Surface,
        source_note: Some("词面".into()),
    }];
    if let Some(b) = base {
        if b != surface && !b.is_empty() && b != "*" {
            v.push(CandidateForm {
                form: b.to_string(),
                form_kind: FormKind::Base,
                source_note: Some("还原形".into()),
            });
        }
    }
    if let Some(r) = reading {
        if !r.is_empty() && r != "*" {
            v.push(CandidateForm {
                form: r.to_string(),
                form_kind: FormKind::Reading,
                source_note: Some("读音".into()),
            });
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Connection, std::path::PathBuf) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "gll-dictq-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dict_path = dir.join("dict.db");
        let c = Connection::open(&dict_path).unwrap();
        c.execute_batch(
            "CREATE TABLE dict_entry(entry_id INTEGER PRIMARY KEY, source TEXT, headword TEXT, reading_kana TEXT, pos_json TEXT, common INTEGER);
             CREATE TABLE dict_index(form TEXT, form_kind TEXT, entry_id INTEGER);
             CREATE TABLE dict_gloss(entry_id INTEGER, lang TEXT, gloss_json TEXT, PRIMARY KEY(entry_id, lang));
             INSERT INTO dict_entry VALUES (1,'zhwiktionary','食べる','たべる','[\"動詞\"]',1);
             INSERT INTO dict_index VALUES ('食べる','kanji',1);
             INSERT INTO dict_index VALUES ('たべる','kana',1);
             INSERT INTO dict_gloss VALUES (1,'zh','[\"吃\"]');
             INSERT INTO dict_entry VALUES (2,'jmdict','ドギマゴ',NULL,'[]',0);
             INSERT INTO dict_index VALUES ('ドギマゴ','kana',2);
             INSERT INTO dict_gloss VALUES (2,'en','[\"doggym\"]');
             ",
        )
        .unwrap();
        let conn = Connection::open(dir.join("app.db")).unwrap();
        crate::DictMount::mount(&conn, &dict_path).unwrap();
        // 建术语表并注入一条。
        conn.execute_batch(
            "CREATE TABLE term(term_id INTEGER PRIMARY KEY AUTOINCREMENT, source TEXT NOT NULL, note TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
             CREATE TABLE term_text(term_id INTEGER NOT NULL, lang TEXT NOT NULL, text TEXT NOT NULL, PRIMARY KEY(term_id, lang));
             INSERT INTO term (source, created_at, updated_at) VALUES ('user', 0, 0);
             INSERT INTO term_text VALUES (1, 'jp', 'ファデュイ');
             INSERT INTO term_text VALUES (1, 'chs', '愚人众');",
        )
        .unwrap();
        (conn, dir)
    }

    #[test]
    fn term_entry_ranked_first() {
        let (conn, dir) = setup();
        let r = DictSearchService::search(
            &conn,
            &vec![CandidateForm { form: "ファデュイ".into(), form_kind: FormKind::Surface, source_note: None }],
        )
        .unwrap();
        assert!(r.dict_available);
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].source, DictSource::Term);
        let texts = r.entries[0].term_texts.clone().unwrap();
        assert!(texts.iter().any(|t| t.lang == "chs" && t.text == "愚人众"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn base_form_candidate_hits_and_merged() {
        let (conn, dir) = setup();
        // 活用形「食べた」exact 未命中 → 还原形「食べる」候选命中。
        let cands = vec![
            CandidateForm { form: "食べた".into(), form_kind: FormKind::Surface, source_note: Some("输入原文".into()) },
            CandidateForm { form: "食べる".into(), form_kind: FormKind::Base, source_note: Some("按原形命中".into()) },
        ];
        let r = DictSearchService::search(&conn, &cands).unwrap();
        assert!(r.entries.iter().any(|e| e.headword == "食べる" && e.matched_form_kind == FormKind::Base));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unmounted_returns_unavailable() {
        let conn = Connection::open_in_memory().unwrap();
        let r = DictSearchService::search(&conn, &[CandidateForm { form: "x".into(), form_kind: FormKind::Surface, source_note: None }]).unwrap();
        assert!(!r.dict_available);
        assert!(r.entries.is_empty());
    }
}
