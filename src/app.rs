use gloo_net::http::Request;
use leptos::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, CustomEvent, CustomEventInit, EventTarget};

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaperMetadata {
    title: String,
    year: Option<i32>,
    topic: Option<String>,
    abstract_text: Option<String>,
    pdf_url: Option<String>,
    citation_count: u64,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct GraphStats {
    nodes: usize,
    links: usize,
    mode: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IngestionSummary {
    works_ingested: usize,
    citation_links: usize,
    truncated: bool,
}

#[derive(Serialize)]
struct IngestRequest {
    doi: String,
    depth: u8,
}

async fn fetch_paper(id: &str) -> Result<PaperMetadata, String> {
    let response = Request::get(&format!("/api/v1/citations/paper/{id}"))
        .send()
        .await
        .map_err(|error| format!("Paper request failed: {error}"))?;
    if !response.ok() {
        return Err(format!(
            "Paper metadata returned HTTP {}",
            response.status()
        ));
    }
    response
        .json::<PaperMetadata>()
        .await
        .map_err(|error| format!("Invalid paper metadata: {error}"))
}

async fn ingest_network(identifier: String, depth: u8) -> Result<IngestionSummary, String> {
    let payload = serde_json::to_string(&IngestRequest {
        doi: identifier,
        depth,
    })
    .map_err(|error| format!("Unable to prepare ingestion request: {error}"))?;
    let response = Request::post("/api/v1/citations/ingest")
        .header("content-type", "application/json")
        .body(payload)
        .map_err(|error| format!("Unable to build ingestion request: {error}"))?
        .send()
        .await
        .map_err(|error| format!("Ingestion request failed: {error}"))?;
    if !response.ok() {
        let message = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown ingestion error".to_owned());
        return Err(message);
    }
    response
        .json::<IngestionSummary>()
        .await
        .map_err(|error| format!("Invalid ingestion response: {error}"))
}

fn dispatch_macro_event(name: &str, detail: &str) {
    let Some(window) = window() else {
        return;
    };
    let options = CustomEventInit::new();
    options.set_detail(&JsValue::from_str(detail));
    if let Ok(event) = CustomEvent::new_with_event_init_dict(name, &options) {
        let _ = window.dispatch_event(&event);
    }
}

#[component]
pub fn App() -> impl IntoView {
    let (selected_paper, set_selected_paper) = create_signal::<Option<PaperMetadata>>(None);
    let (paper_loading, set_paper_loading) = create_signal(false);
    let (graph_stats, set_graph_stats) = create_signal(GraphStats::default());
    let (graph_mode, set_graph_mode) = create_signal("citations".to_owned());
    let (year_filter, set_year_filter) = create_signal("All publication years".to_owned());
    let (gesture_status, set_gesture_status) = create_signal("Gesture camera is off".to_owned());
    let (identifier, set_identifier) = create_signal(String::new());
    let (depth, set_depth) = create_signal(1_u8);
    let (ingesting, set_ingesting) = create_signal(false);
    let (notice, set_notice) = create_signal::<Option<String>>(None);

    if let Some(browser_window) = window() {
        let event_target: EventTarget = browser_window.into();

        let selected_callback =
            Closure::<dyn FnMut(CustomEvent)>::new(move |event: CustomEvent| {
                let id: String = match event.detail().as_string() {
                    Some(value) => value,
                    None => return,
                };
                set_paper_loading.set(true);
                spawn_local(async move {
                    match fetch_paper(&id).await {
                        Ok(paper) => {
                            set_selected_paper.set(Some(paper));
                            set_notice.set(None);
                        }
                        Err(error) => set_notice.set(Some(error)),
                    }
                    set_paper_loading.set(false);
                });
            });
        let stats_callback = Closure::<dyn FnMut(CustomEvent)>::new(move |event: CustomEvent| {
            let payload: Option<String> = event.detail().as_string();
            if let Some(payload) = payload {
                if let Ok(stats) = serde_json::from_str::<GraphStats>(&payload) {
                    set_graph_stats.set(stats);
                }
            }
        });
        let gesture_callback = Closure::<dyn FnMut(CustomEvent)>::new(move |event: CustomEvent| {
            if let Some(status) = event.detail().as_string() {
                set_gesture_status.set(status);
            }
        });
        let filter_callback = Closure::<dyn FnMut(CustomEvent)>::new(move |event: CustomEvent| {
            if let Some(filter) = event.detail().as_string() {
                set_year_filter.set(filter);
            }
        });

        let _ = event_target.add_event_listener_with_callback(
            "citation-node-selected",
            selected_callback.as_ref().unchecked_ref(),
        );
        let _ = event_target.add_event_listener_with_callback(
            "citation-graph-loaded",
            stats_callback.as_ref().unchecked_ref(),
        );
        let _ = event_target.add_event_listener_with_callback(
            "citation-gesture-status",
            gesture_callback.as_ref().unchecked_ref(),
        );
        let _ = event_target.add_event_listener_with_callback(
            "citation-year-filter",
            filter_callback.as_ref().unchecked_ref(),
        );

        on_cleanup(move || {
            let _ = event_target.remove_event_listener_with_callback(
                "citation-node-selected",
                selected_callback.as_ref().unchecked_ref(),
            );
            let _ = event_target.remove_event_listener_with_callback(
                "citation-graph-loaded",
                stats_callback.as_ref().unchecked_ref(),
            );
            let _ = event_target.remove_event_listener_with_callback(
                "citation-gesture-status",
                gesture_callback.as_ref().unchecked_ref(),
            );
            let _ = event_target.remove_event_listener_with_callback(
                "citation-year-filter",
                filter_callback.as_ref().unchecked_ref(),
            );
        });
    }

    let ingest = move |_| {
        let requested_identifier = identifier.get().trim().to_owned();
        if requested_identifier.is_empty() || ingesting.get_untracked() {
            set_notice.set(Some("Enter a DOI or arXiv identifier first.".to_owned()));
            return;
        }
        set_ingesting.set(true);
        set_notice.set(Some(
            "Reading the citation neighborhood from OpenAlex…".to_owned(),
        ));
        let requested_depth = depth.get_untracked();
        spawn_local(async move {
            match ingest_network(requested_identifier, requested_depth).await {
                Ok(summary) => {
                    let suffix = if summary.truncated {
                        " The safety limit was reached."
                    } else {
                        ""
                    };
                    set_notice.set(Some(format!(
                        "Ingested {} papers and {} citation links.{suffix}",
                        summary.works_ingested, summary.citation_links
                    )));
                    dispatch_macro_event("citation-graph-reload", "");
                }
                Err(error) => set_notice.set(Some(error)),
            }
            set_ingesting.set(false);
        });
    };

    view! {
        <main id="app-shell">
            <header class="topbar">
                <div class="brand-lockup">
                    <div>
                        <h1>"MeshGraph"</h1>
                        <p>"Research Citation Constellation"</p>
                    </div>
                </div>
                <form class="ingest-form" on:submit=move |event| {
                    event.prevent_default();
                    ingest(event);
                }>
                    <input
                        aria-label="DOI or arXiv identifier"
                        placeholder="DOI or arXiv ID"
                        prop:value=identifier
                        on:input=move |event| set_identifier.set(event_target_value(&event))
                    />
                    <select
                        aria-label="Citation depth"
                        on:change=move |event| {
                            set_depth.set(event_target_value(&event).parse().unwrap_or(1));
                        }
                    >
                        <option value="1">"1 hop"</option>
                        <option value="2">"2 hops"</option>
                        <option value="3">"3 hops"</option>
                    </select>
                    <button type="submit" disabled=ingesting>
                        {move || if ingesting.get() { "Mapping…" } else { "Map constellation" }}
                    </button>
                </form>
                <button
                    id="gesture-toggle"
                    class="secondary-button"
                    type="button"
                    on:click=move |_| dispatch_macro_event("citation-gesture-toggle", "")
                >
                    "Hand gestures"
                </button>
            </header>

            <section class="hud-panel graph-overview" aria-label="Graph overview">
                <span class="eyebrow">"LIVE RDF TOPOLOGY"</span>
                <strong>{move || format!("{} papers", graph_stats.get().nodes)}</strong>
                <span>{move || format!("{} relationships", graph_stats.get().links)}</span>
                <span>{move || format!("{} topology", graph_stats.get().mode)}</span>
                <span>{move || year_filter.get()}</span>
                <span class="gesture-state">{move || gesture_status.get()}</span>
            </section>

            <nav class="mode-switcher" aria-label="Topology mode">
                {[
                    ("citations", "Citations"),
                    ("co-citation", "Co-citation"),
                    ("bibliographic-coupling", "Coupling"),
                ]
                .into_iter()
                .map(|(mode, label)| view! {
                    <button
                        type="button"
                        class:active=move || graph_mode.get() == mode
                        on:click=move |_| {
                            set_graph_mode.set(mode.to_owned());
                            set_selected_paper.set(None);
                        }
                    >{label}</button>
                })
                .collect_view()}
            </nav>

            <Show when=move || notice.get().is_some()>
                <div class="notice" role="status">
                    {move || notice.get().unwrap_or_default()}
                    <button type="button" aria-label="Dismiss" on:click=move |_| set_notice.set(None)>
                        "×"
                    </button>
                </div>
            </Show>

            <Show when=move || selected_paper.get().is_some() || paper_loading.get()>
                <aside id="paper-panel" class="hud-panel paper-panel">
                    <button
                        class="panel-close"
                        type="button"
                        aria-label="Close paper details"
                        on:click=move |_| {
                            set_selected_paper.set(None);
                            dispatch_macro_event("citation-selection-clear", "");
                        }
                    >"×"</button>
                    <Show
                        when=move || !paper_loading.get()
                        fallback=|| view! { <p class="loading">"Resolving RDF metadata…"</p> }
                    >
                        {move || selected_paper.get().map(|paper| view! {
                            <span class="eyebrow">"Article"</span>
                            <h2>{paper.title.clone()}</h2>
                            <div class="paper-facts">
                                <span>{paper.year.map(|year| year.to_string()).unwrap_or_else(|| "Year unknown".to_owned())}</span>
                                <span>{paper.topic.clone().unwrap_or_else(|| "Unclassified".to_owned())}</span>
                                <span>{format!("{} citations", paper.citation_count)}</span>
                            </div>
                            <p>{paper.abstract_text.clone().unwrap_or_else(|| "No abstract is available in OpenAlex for this record.".to_owned())}</p>
                            <div class="paper-links">
                                <Show when={
                                    let paper = paper.clone();
                                    move || paper.pdf_url.is_some()
                                }>
                                    <a href=paper.pdf_url.clone().unwrap_or_default() target="_blank" rel="noreferrer">"Open paper"</a>
                                </Show>
                            </div>
                        })}
                    </Show>
                </aside>
            </Show>

            <a-scene
                id="citation-scene"
                background="color: #e8edf2"
                cursor="rayOrigin: mouse"
                raycaster="objects: .citation-graph"
                renderer="colorManagement: true; antialias: true; physicallyCorrectLights: true"
                webxr="requiredFeatures: local-floor; optionalFeatures: bounded-floor, hand-tracking; referenceSpaceType: local-floor"
                vr-mode-ui="enabled: true"
                gesture-controls="worker: /public/hand-worker.js?v=0.10.35-2; graph: #citation-graph; rig: #rig"
            >
                <a-entity
                    id="citation-graph"
                    class="citation-graph"
                    position="0 1 -20"
                    af-force-graph=move || format!(
                        "endpoint: /api/v1/citations/graph?mode={}; maxNodes: 12000",
                        graph_mode.get()
                    )
                ></a-entity>

                <a-entity id="rig" position="0 1.6 7" vr-locomotion>
                    <a-camera
                        id="research-camera"
                        look-controls="pointerLockEnabled: false"
                        wasd-controls="acceleration: 28"
                        camera="fov: 65"
                    ></a-camera>
                    <a-entity
                        id="left-controller"
                        laser-controls="hand: left"
                        raycaster="objects: .citation-graph"
                    ></a-entity>
                    <a-entity
                        id="right-controller"
                        laser-controls="hand: right"
                        raycaster="objects: .citation-graph"
                    ></a-entity>
                </a-entity>

                <a-entity light="type: ambient; color: #dce8f5; intensity: 1.15"></a-entity>
                <a-entity light="type: directional; color: #fff4df; intensity: 1.6" position="-4 8 6"></a-entity>
                <a-sky color="#e8edf2"></a-sky>
            </a-scene>

            <footer class="interaction-hint">
                "PINCH SELECT · OPEN PALM ORBIT · TWO-HAND SPREAD ZOOM · INDEX SWEEP FILTER"
            </footer>
        </main>
    }
}
