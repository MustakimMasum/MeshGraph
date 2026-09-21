use gloo_net::http::Request;
use leptos::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;
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
    doi: Option<String>,
    pdf_url: Option<String>,
    citation_count: u64,
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
    let (graph_mode, set_graph_mode) = create_signal("citations".to_owned());
    let (gesture_source, set_gesture_source) = create_signal("webcam".to_owned());
    let (settings_open, set_settings_open) = create_signal(false);
    let (search_open, set_search_open) = create_signal(false);
    let search_input = create_node_ref::<html::Input>();
    let (gesture_status, set_gesture_status) =
        create_signal("Hand input is off · Webcam selected".to_owned());
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
        let gesture_callback = Closure::<dyn FnMut(CustomEvent)>::new(move |event: CustomEvent| {
            if let Some(status) = event.detail().as_string() {
                set_gesture_status.set(status);
            }
        });

        let _ = event_target.add_event_listener_with_callback(
            "citation-node-selected",
            selected_callback.as_ref().unchecked_ref(),
        );
        let _ = event_target.add_event_listener_with_callback(
            "citation-gesture-status",
            gesture_callback.as_ref().unchecked_ref(),
        );

        on_cleanup(move || {
            let _ = event_target.remove_event_listener_with_callback(
                "citation-node-selected",
                selected_callback.as_ref().unchecked_ref(),
            );
            let _ = event_target.remove_event_listener_with_callback(
                "citation-gesture-status",
                gesture_callback.as_ref().unchecked_ref(),
            );
        });
    }

    let ingest = move || {
        let requested_identifier = identifier.get().trim().to_owned();
        if ingesting.get_untracked() {
            return;
        }
        if requested_identifier.is_empty() {
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
            <div class="brand-lockup">
                <h1>"MeshGraph"</h1>
                <p>"Research Citation Constellation"</p>
            </div>

            <div class="search-controls" class:open=move || search_open.get()>
                <form
                    class="search-form"
                    aria-hidden=move || (!search_open.get()).to_string()
                    on:submit=move |event| {
                        event.prevent_default();
                        ingest();
                    }
                >
                    <input
                        node_ref=search_input
                        aria-label="DOI or arXiv identifier"
                        placeholder="DOI or arXiv ID"
                        tabindex=move || if search_open.get() { "0" } else { "-1" }
                        prop:value=identifier
                        on:input=move |event| set_identifier.set(event_target_value(&event))
                    />
                </form>
                <button
                    class="input-control"
                    class:active=move || search_open.get()
                    type="button"
                    aria-label="Search"
                    aria-expanded=move || search_open.get().to_string()
                    title="Search"
                    on:click=move |_| {
                        if !search_open.get_untracked() {
                            set_search_open.set(true);
                            set_timeout(
                                move || {
                                    if let Some(input) = search_input.get() {
                                        let _ = input.focus();
                                    }
                                },
                                Duration::ZERO,
                            );
                        } else if identifier.get_untracked().trim().is_empty() {
                            set_search_open.set(false);
                        } else {
                            ingest();
                        }
                    }
                >
                    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none">
                        <circle cx="11" cy="11" r="6.5"></circle>
                        <path d="m16 16 4.5 4.5"></path>
                    </svg>
                </button>
            </div>

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
                        {move || selected_paper.get().map(|paper| {
                            let paper_url = paper.pdf_url.clone().or_else(|| paper.doi.clone());
                            view! {
                                <div class="paper-content">
                                    <span class="eyebrow">"Article"</span>
                                    <h2>{paper.title.clone()}</h2>
                                    <div class="paper-facts">
                                        <span>{paper.year.map(|year| year.to_string()).unwrap_or_else(|| "Year unknown".to_owned())}</span>
                                        <span>{paper.topic.clone().unwrap_or_else(|| "Unclassified".to_owned())}</span>
                                        <span>{format!("{} citations", paper.citation_count)}</span>
                                    </div>
                                    <p class="paper-abstract">{paper.abstract_text.clone().unwrap_or_else(|| "No abstract is available in OpenAlex for this record.".to_owned())}</p>
                                    <div class="paper-links">
                                        <Show when={
                                            let paper_url = paper_url.clone();
                                            move || paper_url.is_some()
                                        }>
                                            <a href=paper_url.clone().unwrap_or_default() target="_blank" rel="noreferrer">"Open paper"</a>
                                        </Show>
                                    </div>
                                </div>
                            }
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
                vr-mode-ui="enabled: false"
                webxr-launcher
                gesture-controls="worker: /public/hand-worker.js?v=0.10.35-4; graph: #citation-graph; rig: #rig"
            >
                <a-entity
                    id="citation-graph"
                    class="citation-graph"
                    position="0 1 -16"
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
                        vertical-controls="speed: 3"
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

            <div id="gesture-pointer" hidden aria-hidden="true"></div>

            <Show when=move || settings_open.get()>
                <aside class="settings-menu" aria-label="Graph settings">
                    <section>
                        <span class="settings-label">"Connection type"</span>
                        <nav class="connection-options" aria-label="Connection type">
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
                    </section>
                    <section>
                        <span class="settings-label">"Citation depth"</span>
                        <div class="depth-options" role="group" aria-label="Citation depth">
                            {[(1_u8, "1 hop"), (2_u8, "2 hops"), (3_u8, "3 hops")]
                                .into_iter()
                                .map(|(hop_count, label)| view! {
                                    <button
                                        type="button"
                                        class:active=move || depth.get() == hop_count
                                        on:click=move |_| set_depth.set(hop_count)
                                    >{label}</button>
                                })
                                .collect_view()}
                        </div>
                    </section>
                    <section class="navigation-guide">
                        <span class="settings-label">"Navigation"</span>
                        <div><kbd>"W A S D"</kbd><span>"Move"</span></div>
                        <div><kbd>"E / C"</kbd><span>"Up / down"</span></div>
                        <div><kbd>"Drag"</kbd><span>"Look around"</span></div>
                        <div><kbd>"Scroll"</kbd><span>"Zoom"</span></div>
                        <div><kbd>"Point + pinch"</kbd><span>"Select"</span></div>
                        <div><kbd>"Open palm"</kbd><span>"Pan"</span></div>
                        <div><kbd>"Quick swipe"</kbd><span>"Rotate"</span></div>
                        <div><kbd>"Two-hand spread"</kbd><span>"Zoom"</span></div>
                    </section>
                    <section class="input-status">
                        <span class="settings-label">"Hand input"</span>
                        <span>{move || gesture_status.get()}</span>
                    </section>
                </aside>
            </Show>

            <div class="input-controls" role="group" aria-label="Input controls">
                <button
                    id="reset-view-button"
                    class="input-control"
                    type="button"
                    aria-label="Refocus selected node"
                    title="Reset view"
                    on:click=move |_| dispatch_macro_event("citation-focus-selected", "")
                >
                    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none">
                        <path d="M4.5 9A8 8 0 1 1 4 14"></path>
                        <path d="M4.5 4.5V9H9"></path>
                    </svg>
                </button>
                <button
                    id="webcam-input-button"
                    class="input-control"
                    class:active=move || gesture_source.get() == "webcam"
                    type="button"
                    aria-label="Use webcam hand input"
                    title="Webcam"
                    on:click=move |_| {
                        set_gesture_source.set("webcam".to_owned());
                        dispatch_macro_event("citation-gesture-toggle", "webcam");
                    }
                >
                    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none">
                        <circle cx="12" cy="10" r="6.5"></circle>
                        <circle cx="12" cy="10" r="2.25"></circle>
                        <path d="M12 16.5v4M8.5 20.5h7"></path>
                    </svg>
                </button>
                <button
                    id="leap-input-button"
                    class="input-control"
                    class:active=move || gesture_source.get() == "hyperion"
                    type="button"
                    aria-label="Use Leap Motion infrared hand input"
                    title="Leap Motion"
                    on:click=move |_| {
                        set_gesture_source.set("hyperion".to_owned());
                        dispatch_macro_event("citation-gesture-toggle", "hyperion");
                    }
                >
                    <span class="ir-mark" aria-hidden="true">"IR"</span>
                </button>
                <button
                    id="enter-vr-button"
                    class="input-control vr-control"
                    type="button"
                    disabled=true
                    aria-label="Enter VR mode"
                    title="Enter VR mode"
                >
                    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none">
                        <path d="M4 6.5h16a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2h-3.1a2 2 0 0 1-1.42-.59l-2.07-2.07a2 2 0 0 0-2.82 0l-2.07 2.07a2 2 0 0 1-1.42.59H4a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2Z"></path>
                        <circle cx="8" cy="11.5" r="2.25"></circle>
                        <circle cx="16" cy="11.5" r="2.25"></circle>
                    </svg>
                </button>
                <button
                    id="settings-button"
                    class="input-control"
                    class:active=move || settings_open.get()
                    type="button"
                    aria-label="Settings"
                    aria-expanded=move || settings_open.get().to_string()
                    title="Settings"
                    on:click=move |_| set_settings_open.update(|open| *open = !*open)
                >
                    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none">
                        <circle cx="12" cy="12" r="3"></circle>
                        <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06a1.7 1.7 0 0 0-1.88-.34 1.7 1.7 0 0 0-1.03 1.56V21h-4v-.08A1.7 1.7 0 0 0 8.95 19.4a1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.56-1.03H3v-4h.08A1.7 1.7 0 0 0 4.6 8.95a1.7 1.7 0 0 0-.34-1.88L4.2 7l2.83-2.83.06.06A1.7 1.7 0 0 0 8.95 4.6 1.7 1.7 0 0 0 9.98 3H14v.08a1.7 1.7 0 0 0 1.03 1.56 1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06a1.7 1.7 0 0 0-.34 1.88 1.7 1.7 0 0 0 1.56 1.03H21v4h-.08A1.7 1.7 0 0 0 19.4 15Z"></path>
                    </svg>
                </button>
                <span id="vr-status" class="visually-hidden" role="status">"Checking WebXR…"</span>
            </div>
        </main>
    }
}
