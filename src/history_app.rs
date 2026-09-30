use crate::history_model::HistoryDataset;
use gloo_net::http::Request;
use leptos::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use wasm_bindgen::{closure::Closure, JsCast};
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, CustomEvent, CustomEventInit, EventTarget};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ViewState {
    selected: String,
    expanded: Vec<String>,
    classification: String,
    tag: String,
    relation: String,
    organizational: bool,
    path: Vec<String>,
}

fn emit(name: &str, detail: Value) {
    if let Some(window) = window() {
        let init = CustomEventInit::new();
        init.set_detail(&wasm_bindgen::JsValue::from_str(
            &detail
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| detail.to_string()),
        ));
        if let Ok(event) = CustomEvent::new_with_event_init_dict(name, &init) {
            let _ = window.dispatch_event(&event);
        }
    }
}

fn eligible(edge: &crate::history_model::HistoryLink, state: &ViewState) -> bool {
    (state.organizational || edge.meaning == 1)
        && (state.relation.is_empty() || edge.kind == state.relation)
}

#[component]
pub fn HistoryApp() -> impl IntoView {
    let data = create_rw_signal::<Option<HistoryDataset>>(None);
    let state = create_rw_signal(ViewState::default());
    let notice = create_rw_signal("Loading JPL History…".to_owned());
    let query = create_rw_signal(String::new());
    let results = create_rw_signal::<Vec<Value>>(vec![]);
    let selected_link = create_rw_signal(String::new());
    let past = create_rw_signal::<Vec<ViewState>>(vec![]);
    let future = create_rw_signal::<Vec<ViewState>>(vec![]);
    let bookmarks = create_rw_signal::<Vec<Value>>(vec![]);
    let bookmark_name = create_rw_signal("My JPL exploration".to_owned());
    let path_from = create_rw_signal(String::new());
    let path_result = create_rw_signal::<Option<Value>>(None);
    let quality_open = create_rw_signal(false);
    let tour_step = create_rw_signal::<Option<usize>>(None);
    let search_generation = create_rw_signal(0_u32);

    let commit = Callback::new(move |next: ViewState| {
        past.update(|items| items.push(state.get_untracked()));
        future.set(vec![]);
        state.set(next);
        selected_link.set(String::new());
    });
    let select = Callback::new(move |id: String| {
        let mut next = state.get_untracked();
        next.selected = id.clone();
        if !next.expanded.contains(&id) {
            next.expanded.push(id);
        }
        commit.call(next);
    });

    if let Some(browser) = window() {
        let target: EventTarget = browser.into();
        let selected = Closure::<dyn FnMut(CustomEvent)>::new(move |event: CustomEvent| {
            if let Some(id) = event.detail().as_string() {
                select.call(id);
            }
        });
        let restored = Closure::<dyn FnMut(CustomEvent)>::new(move |event: CustomEvent| {
            if let Some(raw) = event.detail().as_string() {
                if let Ok(value) = serde_json::from_str::<Value>(&raw) {
                    if let Ok(mut view) = serde_json::from_value::<ViewState>(value["view"].clone())
                    {
                        if let Some(dataset) = data.get_untracked() {
                            let valid: HashSet<_> =
                                dataset.nodes.iter().map(|n| n.id.clone()).collect();
                            let missing = view
                                .expanded
                                .iter()
                                .filter(|id| !valid.contains(*id))
                                .count();
                            view.expanded.retain(|id| valid.contains(id));
                            view.path.retain(|id| valid.contains(id));
                            if !valid.contains(&view.selected) {
                                view.selected = dataset.root_id.clone();
                            }
                            if view.expanded.is_empty() {
                                view.expanded.push(dataset.root_id);
                            }
                            commit.call(view);
                            notice.set(format!(
                                "View restored. {missing} unavailable expanded topics omitted.{}",
                                if value["version"].as_str() != Some(&dataset.version) {
                                    " The dataset version has changed."
                                } else {
                                    ""
                                }
                            ));
                            emit("history-camera-restore", value["camera"].clone());
                        }
                    }
                }
            }
        });
        let saved = Closure::<dyn FnMut(CustomEvent)>::new(move |event: CustomEvent| {
            if let Some(raw) = event.detail().as_string() {
                if let Ok(items) = serde_json::from_str::<Vec<Value>>(&raw) {
                    bookmarks.set(items);
                }
            }
        });
        let status = Closure::<dyn FnMut(CustomEvent)>::new(move |event: CustomEvent| {
            if let Some(text) = event.detail().as_string() {
                notice.set(text);
            }
        });
        let actions =
            Closure::<dyn FnMut(CustomEvent)>::new(move |event: CustomEvent| {
                match event.detail().as_string().as_deref() {
                    Some("home") => {
                        if let Some(d) = data.get_untracked() {
                            commit.call(ViewState {
                                selected: d.root_id.clone(),
                                expanded: vec![d.root_id],
                                ..Default::default()
                            });
                        }
                    }
                    Some("expand") => select.call(state.get_untracked().selected),
                    _ => {}
                }
            });
        for (name, callback) in [
            ("citation-node-selected", selected.as_ref()),
            ("history-restore", restored.as_ref()),
            ("history-bookmarks", saved.as_ref()),
            ("citation-gesture-status", status.as_ref()),
            ("history-xr-action", actions.as_ref()),
        ] {
            let _ = target.add_event_listener_with_callback(name, callback.unchecked_ref());
        }
        on_cleanup(move || {
            for (name, callback) in [
                ("citation-node-selected", selected.as_ref()),
                ("history-restore", restored.as_ref()),
                ("history-bookmarks", saved.as_ref()),
                ("citation-gesture-status", status.as_ref()),
                ("history-xr-action", actions.as_ref()),
            ] {
                let _ = target.remove_event_listener_with_callback(name, callback.unchecked_ref());
            }
        });
    }

    spawn_local(async move {
        match Request::get("/api/v1/history/graph").send().await {
            Ok(response) if response.ok() => match response.json::<HistoryDataset>().await {
                Ok(dataset) => {
                    state.set(ViewState {
                        selected: dataset.root_id.clone(),
                        expanded: vec![dataset.root_id.clone()],
                        ..Default::default()
                    });
                    notice.set(format!(
                        "{} topics · {} connections · incomplete research archive",
                        dataset.nodes.len(),
                        dataset.links.len()
                    ));
                    data.set(Some(dataset));
                    emit("history-bookmark-command", json!({"action": "list"}));
                }
                Err(error) => notice.set(format!("Cannot read history: {error}")),
            },
            Ok(response) => notice.set(
                response
                    .text()
                    .await
                    .unwrap_or_else(|_| "History service unavailable".into()),
            ),
            Err(error) => notice.set(format!("History service unavailable: {error}")),
        }
    });

    create_effect(move |_| {
        if let Some(dataset) = data.get() {
            let current = state.get();
            let mut ids: HashSet<_> = current.expanded.iter().cloned().collect();
            for edge in dataset.links.iter().filter(|e| eligible(e, &current)) {
                if current.expanded.contains(&edge.source) {
                    ids.insert(edge.target.clone());
                }
                if current.expanded.contains(&edge.target) {
                    ids.insert(edge.source.clone());
                }
            }
            ids.extend(current.path.clone());
            let nodes: Vec<_> = dataset
                .nodes
                .iter()
                .filter(|n| {
                    ids.contains(&n.id)
                        && (current.organizational || n.kind == 1)
                        && (current.classification.is_empty()
                            || n.types.contains(&current.classification))
                        && (current.tag.is_empty() || n.tags.contains(&current.tag))
                })
                .collect();
            let visible: HashSet<_> = nodes.iter().map(|n| &n.id).collect();
            let edges: Vec<_> = dataset
                .links
                .iter()
                .filter(|e| {
                    visible.contains(&e.source)
                        && visible.contains(&e.target)
                        && eligible(e, &current)
                })
                .collect();
            emit(
                "history-render",
                json!({"mode": "history", "nodes": nodes, "links": edges, "selected": current.selected, "path": current.path}),
            );
            if let Some(node) = dataset.nodes.iter().find(|n| n.id == current.selected) {
                let connections: Vec<_> = dataset
                    .links
                    .iter()
                    .filter(|e| e.source == node.id || e.target == node.id)
                    .take(6)
                    .map(|e| {
                        let id = if e.source == node.id {
                            &e.target
                        } else {
                            &e.source
                        };
                        format!(
                            "{}: {}",
                            e.kind,
                            dataset
                                .nodes
                                .iter()
                                .find(|n| &n.id == id)
                                .map(|n| n.title.as_str())
                                .unwrap_or("Unknown")
                        )
                    })
                    .collect();
                emit(
                    "history-detail",
                    json!({"title": node.title, "notes": node.notes, "connections": connections}),
                );
            }
        }
    });

    let run_search = move || {
        search_generation.update(|v| *v += 1);
        let generation = search_generation.get_untracked();
        let q = query.get_untracked();
        // Percent-encode UTF-8 without allowing the query to alter route parameters.
        let encoded: String = q.bytes().map(|b| format!("%{b:02X}")).collect();
        spawn_local(async move {
            match Request::get(&format!("/api/v1/history/search?q={encoded}&limit=100"))
                .send()
                .await
            {
                Ok(response) if response.ok() => {
                    if let Ok(value) = response.json::<Value>().await {
                        if generation == search_generation.get_untracked() {
                            results.set(value["results"].as_array().cloned().unwrap_or_default());
                            notice.set(format!(
                                "{} search matches (showing up to 100)",
                                value["total"]
                            ));
                        }
                    }
                }
                _ => notice.set("Search failed. Enter a name or note phrase and retry.".into()),
            }
        });
    };
    let advance_tour = move || {
        if let Some(dataset) = data.get_untracked() {
            let titles = [
                "JPL Computer Graphics Laboratory",
                "People",
                "James F. Blinn",
                "People",
                "JPL Computer Graphics Laboratory",
                "Stories",
                "a galaxy for Man and the Cosmos",
            ];
            let route: Vec<_> = titles
                .iter()
                .filter_map(|title| dataset.nodes.iter().find(|n| n.title == *title))
                .collect();
            if route.len() != titles.len()
                || !route.windows(2).all(|pair| {
                    dataset.links.iter().any(|e| {
                        (e.source == pair[0].id && e.target == pair[1].id)
                            || (e.target == pair[0].id && e.source == pair[1].id)
                    })
                })
            {
                notice.set("The guided route cannot be resolved against this dataset.".into());
                return;
            }
            let index = tour_step
                .get_untracked()
                .map_or(0, |i| (i + 1) % route.len());
            tour_step.set(Some(index));
            select.call(route[index].id.clone());
            notice.set(format!("Editorial tour · stop {}/{}: {}. Inspect the recorded connections and available notes.", index + 1, route.len(), route[index].title));
        }
    };

    view! {
        <main id="history-app">
            <aside id="history-panel">
                <header><a href="/">"← Citations"</a><span class="history-eyebrow">"MESHGRAPH / ARCHIVE"</span><h1>"JPL History"</h1><p>"People, work, and memories of the Computer Graphics Laboratory."</p></header>
                <p id="history-status" role="status">{move || notice.get()}</p>
                <nav class="history-actions" aria-label="Explore history">
                    <button on:click=move |_| { if let Some(d) = data.get_untracked() { commit.call(ViewState { selected: d.root_id.clone(), expanded: vec![d.root_id], ..Default::default() }); } }>"Home"</button>
                    <button disabled=move || past.get().is_empty() on:click=move |_| { let mut items = past.get_untracked(); if let Some(previous) = items.pop() { future.update(|f| f.push(state.get_untracked())); past.set(items); state.set(previous); } }>"Back"</button>
                    <button disabled=move || future.get().is_empty() on:click=move |_| { let mut items = future.get_untracked(); if let Some(next) = items.pop() { past.update(|p| p.push(state.get_untracked())); future.set(items); state.set(next); } }>"Forward"</button>
                    <button on:click=move |_| advance_tour()>{move || if tour_step.get().is_some() { "Next tour stop" } else { "Guided introduction" }}</button>
                </nav>
                <nav class="history-trail" aria-label="Visited topics">{move || {
                    let dataset = data.get();
                    past.get().into_iter().rev().take(5).collect::<Vec<_>>().into_iter().rev().map(|view| {
                        let id = view.selected;
                        let title = dataset.as_ref().and_then(|d| d.nodes.iter().find(|n| n.id == id)).map(|n| n.title.clone()).unwrap_or_default();
                        view! { <button on:click=move |_| select.call(id.clone())>{title}</button><span>" › "</span> }
                    }).collect_view()
                }}</nav>
                <form class="history-search" on:submit=move |event| { event.prevent_default(); run_search(); }>
                    <input aria-label="Search history" placeholder="Search names and notes" prop:value=move || query.get() on:input=move |e| query.set(event_target_value(&e))/>
                    <button type="submit">"Search"</button>
                </form>
                <div id="history-results">{move || results.get().into_iter().map(|item| {
                    let id = item["id"].as_str().unwrap_or_default().to_owned();
                    let title = item["title"].as_str().unwrap_or_default().to_owned();
                    let excerpt = item["excerpt"].as_str().unwrap_or_default().to_owned();
                    let short = id.chars().take(8).collect::<String>();
                    view! { <button class="history-result" on:click=move |_| select.call(id.clone())><strong>{title}</strong><small>{short}</small><span>{excerpt}</span></button> }
                }).collect_view()}</div>
                <details><summary>"Filter this view"</summary>
                    <label>"Type"<select aria-label="History type" prop:value=move || state.get().classification on:change=move |e| { let mut s = state.get_untracked(); s.classification = event_target_value(&e); commit.call(s); }><option value="">"All types"</option>{move || {
                        let mut types: Vec<_> = data.get().map(|d| d.nodes.into_iter().flat_map(|n| n.types).collect()).unwrap_or_default(); types.sort(); types.dedup(); types.into_iter().map(|t| view! { <option value=t.clone()>{t}</option> }).collect_view()
                    }}</select></label>
                    <label>"Tag"<select aria-label="History tag" prop:value=move || state.get().tag on:change=move |e| { let mut s = state.get_untracked(); s.tag = event_target_value(&e); commit.call(s); }><option value="">"All tags"</option>{move || {
                        let mut tags: Vec<_> = data.get().map(|d| d.nodes.into_iter().flat_map(|n| n.tags).collect()).unwrap_or_default(); tags.sort(); tags.dedup(); tags.into_iter().map(|t| view! { <option value=t.clone()>{t}</option> }).collect_view()
                    }}</select></label>
                    <label>"Connections"<select aria-label="History relationship" prop:value=move || state.get().relation on:change=move |e| { let mut s = state.get_untracked(); s.relation = event_target_value(&e); commit.call(s); }><option value="">"All relationships"</option><option>"Hierarchy"</option><option>"Cross-link"</option></select></label>
                    <label><input type="checkbox" prop:checked=move || state.get().organizational on:change=move |e| { let mut s = state.get_untracked(); s.organizational = event_target_checked(&e); commit.call(s); }/>"Show type, tag, and organizational records"</label>
                    <button on:click=move |_| { let mut s = state.get_untracked(); s.classification.clear(); s.tag.clear(); s.relation.clear(); s.organizational = true; commit.call(s); }>"Reveal hidden selection"</button>
                </details>
                {move || data.get().and_then(|d| d.nodes.iter().find(|n| n.id == state.get().selected).cloned().map(|node| {
                    let id = node.id.clone();
                    let expand_id = id.clone();
                    let collapse_id = id.clone();
                    let origin_id = id.clone();
                    let attachments = d.attachments.iter().filter(|a| a.owner == id).cloned().collect::<Vec<_>>();
                    let connections = d.links.iter().filter(|l| l.source == id || l.target == id).cloned().collect::<Vec<_>>();
                    view! { <section id="history-node"><h2>{node.title}</h2><p class="history-meta">{format!("{} · {}", node.topic, node.id)}</p>
                        <div class="history-actions"><button on:click=move |_| select.call(expand_id.clone())>"Expand neighbors"</button><button on:click=move |_| { let mut s = state.get_untracked(); s.expanded.retain(|id| id != &collapse_id); commit.call(s); }>"Collapse branch"</button><button on:click=move |_| emit("citation-focus-selected", Value::Null)>"Focus"</button></div>
                        <h3>"Notes"</h3><p class="history-note">{if node.notes.is_empty() { "No written note supplied for this topic.".into() } else { node.notes }}</p>
                        <h3>"Sources & attachments"</h3>{attachments.into_iter().map(|a| {
                            let url = if a.status == "external-url" { a.location.clone() } else { format!("/api/v1/history/attachments/{}", a.id) };
                            let available = a.status == "available" || a.status == "external-url";
                            view! { <div class="history-attachment">{if available { view! { <a href=url target="_blank" rel="noopener noreferrer">{a.name}</a> }.into_view() } else { view! { <span>{a.name}</span> }.into_view() }}<small>{a.status}</small></div> }
                        }).collect_view()}
                        <h3>"Connections"</h3>{connections.into_iter().map(|edge| {
                            let other = if edge.source == id { edge.target.clone() } else { edge.source.clone() };
                            let name = d.nodes.iter().find(|n| n.id == other).map(|n| n.title.clone()).unwrap_or_else(|| other.clone());
                            let link_id = edge.id.clone();
                            view! { <div class="history-connection"><button on:click=move |_| select.call(other.clone())>{name}</button><button aria-label="Inspect relationship" on:click=move |_| selected_link.set(link_id.clone())>{format!("{} · {}", edge.kind, edge.meaning_label)}</button></div> }
                        }).collect_view()}
                        <button on:click=move |_| { path_from.set(origin_id.clone()); notice.set("Path start set. Select another topic, then Find path.".into()); }>"Use as path start"</button>
                        <details><summary>"Original source record"</summary><pre>{serde_json::to_string_pretty(&node.raw).unwrap_or_default()}</pre></details>
                    </section> }
                }))}
                {move || data.get().and_then(|d| d.links.iter().find(|l| l.id == selected_link.get()).cloned().map(|edge| {
                    let title = |id: &str| d.nodes.iter().find(|n| n.id == id).map(|n| n.title.clone()).unwrap_or_else(|| id.to_owned());
                    view! { <section id="history-link"><h3>"Relationship inspector"</h3><p>{format!("{} ↔ {}", title(&edge.source), title(&edge.target))}</p><p>{format!("{} · {} · {}", edge.kind, edge.meaning_label, edge.name)}</p><p>{edge.direction_label}</p><details><summary>"Original relationship record"</summary><pre>{serde_json::to_string_pretty(&edge.raw).unwrap_or_default()}</pre></details></section> }
                }))}
                <details><summary>"Find a connection path"</summary><p>{move || format!("Start: {}", path_from.get())}</p><button disabled=move || path_from.get().is_empty() on:click=move |_| {
                    let from = path_from.get_untracked(); let to = state.get_untracked().selected;
                    spawn_local(async move { match Request::get(&format!("/api/v1/history/path?from={from}&to={to}")).send().await {
                        Ok(response) if response.ok() => if let Ok(result) = response.json::<Value>().await {
                            let mut s = state.get_untracked(); s.path = result["nodes"].as_array().map(|v| v.iter().filter_map(|n| n.as_str().map(str::to_owned)).collect()).unwrap_or_default();
                            s.classification.clear(); s.tag.clear(); s.relation.clear(); commit.call(s); path_result.set(Some(result));
                        }, _ => notice.set("Unable to find the connection path.".into())
                    }});
                }>"Find path to selected topic"</button>{move || path_result.get().map(|v| {
                    let dataset = data.get();
                    let steps = v["steps"].as_array().cloned().unwrap_or_default();
                    view! { <p>{if v["found"] == true { "Path highlighted. These are recorded connections, not evidence of causation." } else { "No ordinary hierarchy/cross-link path connects these topics." }}</p>
                        <ol>{steps.into_iter().map(|step| {
                            let id = step["linkId"].as_str().unwrap_or_default().to_owned();
                            let label = |key: &str| dataset.as_ref().and_then(|d| d.nodes.iter().find(|n| Some(n.id.as_str()) == step[key].as_str())).map(|n| n.title.clone()).unwrap_or_default();
                            let text = format!("{} ↔ {}", label("from"), label("to"));
                            view! { <li><button on:click=move |_| selected_link.set(id.clone())>{text}</button></li> }
                        }).collect_view()}</ol>
                        <p class="history-meta">"Search uses ordinary hierarchy and cross-links in either direction. Type and tag shortcuts are excluded."</p>
                    }
                })}</details>
                <details><summary>"Saved viewpoints"</summary><input aria-label="Viewpoint name" prop:value=move || bookmark_name.get() on:input=move |e| bookmark_name.set(event_target_value(&e))/><button on:click=move |_| { if let Some(d) = data.get_untracked() { emit("history-bookmark-command", json!({"action": "save", "name": bookmark_name.get_untracked(), "view": state.get_untracked(), "version": d.version})); } }>"Save view"</button>
                    {move || bookmarks.get().into_iter().map(|b| {
                        let id = b["id"].as_str().unwrap_or_default().to_owned(); let remove_id = id.clone(); let rename_id = id.clone();
                        let name = b["name"].as_str().unwrap_or("Saved view").to_owned();
                        view! { <div class="history-actions"><button on:click=move |_| emit("history-bookmark-command", json!({"action":"restore","id":id}))>{name}</button><button on:click=move |_| emit("history-bookmark-command", json!({"action":"rename","id":rename_id,"name":bookmark_name.get_untracked()}))>"Rename"</button><button on:click=move |_| emit("history-bookmark-command", json!({"action":"delete","id":remove_id}))>"Delete"</button></div> }
                    }).collect_view()}
                </details>
                <button on:click=move |_| quality_open.update(|open| *open = !*open)>"Archive quality report"</button>
                <Show when=move || quality_open.get()>{move || data.get().map(|d| {
                    let mut counts = std::collections::BTreeMap::<String, usize>::new();
                    for warning in d.quality["warnings"].as_array().into_iter().flatten() { *counts.entry(warning["kind"].as_str().unwrap_or("other").to_owned()).or_default() += 1; }
                    let repeated = d.quality["duplicateNames"].as_object().map(|items| items.iter().map(|(name, ids)| format!("{}: {} distinct records", name, ids.as_array().map_or(0, Vec::len))).collect::<Vec<_>>()).unwrap_or_default();
                    view! { <section id="history-quality"><h3>"Archive quality"</h3><p>"All link endpoints resolve. Unknown directions are preserved; repeated names remain separate records."</p>
                        <ul>{counts.into_iter().map(|(kind, count)| view! { <li>{format!("{kind}: {count} items to review")}</li> }).collect_view()}</ul>
                        <h3>"Repeated names"</h3><ul>{repeated.into_iter().map(|text| view! { <li>{text}</li> }).collect_view()}</ul>
                        <h3>"Unavailable attachments"</h3><ul>{d.attachments.into_iter().filter(|a| a.status == "unavailable").map(|a| view! { <li>{a.name}</li> }).collect_view()}</ul>
                        <a href="/api/v1/history/quality" target="_blank" rel="noopener noreferrer">"Open full audit report"</a>
                    </section> }
                })}</Show>
                <p class="history-meta">"Spatial depth is layout only. Direction is unresolved where noted. Editorial tours organize the supplied material."</p>
            </aside>
            <div class="history-inputs"><button id="enter-vr-button">"Enter VR"</button><span id="vr-status" role="status">"Checking WebXR…"</span><button on:click=move |_| emit("citation-gesture-toggle", json!("webcam"))>"Webcam hands"</button><button on:click=move |_| emit("citation-gesture-toggle", json!("hyperion"))>"Leap hands"</button></div>
            <a-scene id="history-scene" background="color: #e8edf2" cursor="rayOrigin: mouse" raycaster="objects: .citation-graph, .history-xr-button" renderer="colorManagement: true; antialias: true" vr-mode-ui="enabled: false" webxr-launcher history-bridge gesture-controls="worker: /public/hand-worker.js?v=0.10.35-4; graph: #citation-graph; rig: #rig" webxr="requiredFeatures: local-floor; optionalFeatures: bounded-floor; referenceSpaceType: local-floor">
                <a-entity id="citation-graph" class="citation-graph" position="0 1 -16" af-force-graph="manual: true"></a-entity>
                <a-entity id="rig" position="0 1.6 7" vr-locomotion><a-camera id="research-camera" look-controls="pointerLockEnabled: false" wasd-controls="acceleration: 28" vertical-controls="speed: 3" camera="fov: 65"></a-camera><a-entity laser-controls="hand: left" raycaster="objects: .citation-graph, .history-xr-button"></a-entity><a-entity laser-controls="hand: right" raycaster="objects: .citation-graph, .history-xr-button"></a-entity></a-entity>
                <a-entity light="type: ambient; intensity: 1.2"></a-entity><a-entity light="type: directional; intensity: 1.4" position="-4 8 6"></a-entity>
            </a-scene>
            <div id="gesture-pointer" hidden></div>
        </main>
    }
}
