use gloo_net::http::Request;
use leptos::*;
use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;

#[derive(Clone)]
struct StructureLoad {
    bindings: Vec<StructureBinding>,
    notice: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct StructureBinding {
    #[serde(rename = "partName")]
    part_name: SparqlValue,
    x: SparqlValue,
    y: SparqlValue,
    z: SparqlValue,
}

#[derive(Clone, Debug, Deserialize)]
struct SparqlValue {
    value: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct ComponentMetadata {
    component_name: String,
    display_name: String,
    category: Option<String>,
    purpose: Option<String>,
    power_requirement: Option<String>,
    mission_note: Option<String>,
}

async fn fetch_structure() -> Result<StructureLoad, String> {
    let response = Request::get("/api/v1/structure")
        .send()
        .await
        .map_err(|error| format!("Gateway request failed: {error}"))?;

    if !response.ok() {
        return Ok(StructureLoad {
            bindings: demo_structure(),
            notice: Some("Oxigraph unavailable; using bundled demo coordinates.".to_owned()),
        });
    }

    let bindings = response
        .json::<Vec<StructureBinding>>()
        .await
        .map_err(|error| format!("Invalid gateway response: {error}"))?;

    Ok(StructureLoad {
        bindings,
        notice: None,
    })
}

fn demo_structure() -> Vec<StructureBinding> {
    [
        ("BodyAssembly", "0.0", "-0.3", "0.0"),
        ("Antenna", "-2.6", "-0.3", "0.0"),
        ("RightSuspension", "0.0", "-0.9", "2.2"),
        ("LeftSuspension", "0.0", "-0.9", "-2.2"),
        ("WheelRearRight", "-1.4", "-1.0", "2.8"),
        ("WheelMiddleRight", "0.0", "-1.0", "2.8"),
        ("WheelFrontRight", "1.4", "-1.0", "2.8"),
        ("WheelRearLeft", "-1.4", "-1.0", "-2.8"),
        ("WheelMiddleLeft", "0.0", "-1.0", "-2.8"),
        ("WheelFrontLeft", "1.4", "-1.0", "-2.8"),
        ("Drill", "3.2", "-0.4", "0.0"),
    ]
    .into_iter()
    .map(|(part_name, x, y, z)| StructureBinding {
        part_name: SparqlValue {
            value: part_name.to_owned(),
        },
        x: SparqlValue {
            value: x.to_owned(),
        },
        y: SparqlValue {
            value: y.to_owned(),
        },
        z: SparqlValue {
            value: z.to_owned(),
        },
    })
    .collect()
}

fn animate_components(components: &[StructureBinding], exploded: bool) {
    let Some(document) = window().and_then(|window| window.document()) else {
        return;
    };

    if let Some(assembled_rover) = document.get_element_by_id("assembled-rover") {
        let _ = assembled_rover.set_attribute("visible", if exploded { "false" } else { "true" });
    }
    if let Some(graph_parts) = document.get_element_by_id("graph-parts") {
        let _ = graph_parts.set_attribute("visible", if exploded { "true" } else { "false" });
        let graph_description = components
            .iter()
            .map(|component| {
                format!(
                    "{}:{} {} {}",
                    component.part_name.value,
                    component.x.value,
                    component.y.value,
                    component.z.value
                )
            })
            .collect::<Vec<_>>()
            .join("|");
        let _ = graph_parts.set_attribute("data-graph-parts", &graph_description);
    }

    let exploded_parts = [
        ("ExplodedBody", "0 -0.9 0", "0 -0.9 0"),
        ("ExplodedAntenna", "0 -0.9 0", "-2.6 -0.9 0"),
        ("ExplodedRightWheelAssembly", "0 0 0", "0 0 1.3"),
        ("ExplodedLeftWheelAssembly", "0 0 0", "0 0 -1.3"),
        ("ExplodedDrill", "0 -0.4 0", "3.2 -0.4 0"),
    ];

    for (id, assembled_position, exploded_position) in exploded_parts {
        let Some(element) = document.get_element_by_id(id) else {
            continue;
        };
        let target = if exploded {
            exploded_position
        } else {
            assembled_position
        };
        let animation =
            format!("property: position; to: {target}; dur: 1100; easing: easeOutElastic;");
        let _ = element.set_attribute("animation__position", &animation);
    }
}

fn explode_rover(
    is_exploded: ReadSignal<bool>,
    set_exploded: WriteSignal<bool>,
    components: ReadSignal<Vec<StructureBinding>>,
) {
    if is_exploded.get_untracked() {
        return;
    }

    set_exploded.set(true);
    animate_components(&components.get_untracked(), true);
}

fn set_semantic_selection(
    previous_id: Option<&str>,
    previous_related: &[String],
    element_id: Option<&str>,
) {
    let Some(document) = window().and_then(|window| window.document()) else {
        return;
    };

    if let Some(previous_id) = previous_id {
        if let Some(previous) = document.get_element_by_id(previous_id) {
            let _ = previous.set_attribute("semantic-highlight", "state: none");
        }
    }

    for related_name in previous_related {
        if let Ok(Some(related)) =
            document.query_selector(&format!("[data-component-name='{}']", related_name))
        {
            let _ = related.set_attribute("semantic-highlight", "state: none");
            let related_id = related.id();
            if let Some(tether) = document.get_element_by_id(&format!("tether-{related_id}")) {
                tether.remove();
            }
        }
    }

    let Some(element_id) = element_id else {
        return;
    };

    if let Some(selected) = document.get_element_by_id(element_id) {
        let _ = selected.set_attribute("semantic-highlight", "state: active");
    }
}

fn set_related_highlights(element_id: &str, related_elements: &[String]) {
    let Some(document) = window().and_then(|window| window.document()) else {
        return;
    };
    let Some(scene) = document.get_element_by_id("rover-scene") else {
        return;
    };

    for related_name in related_elements {
        if let Ok(Some(related)) =
            document.query_selector(&format!("[data-component-name='{}']", related_name))
        {
            let _ = related.set_attribute("semantic-highlight", "state: related");
            let related_id = related.id();

            if let Ok(tether) = document.create_element("a-entity") {
                tether.set_id(&format!("tether-{related_id}"));
                let _ = tether.set_attribute(
                    "connection-tether",
                    &format!("source: #{element_id}; target: #{related_id}; color: #ff9900"),
                );
                let _ = scene.append_child(&tether);
            }
        }
    }
}

fn select_component(
    component_name: &'static str,
    element_id: &'static str,
    selected_element: ReadSignal<Option<String>>,
    set_selected_element: WriteSignal<Option<String>>,
    set_metadata: WriteSignal<ComponentMetadata>,
    related_elements: ReadSignal<Vec<String>>,
    set_related_elements: WriteSignal<Vec<String>>,
) {
    let previous = selected_element.get_untracked();
    let previous_related = related_elements.get_untracked();
    set_semantic_selection(previous.as_deref(), &previous_related, Some(element_id));
    set_selected_element.set(Some(element_id.to_owned()));
    set_related_elements.set(vec![]);
    set_metadata.set(ComponentMetadata {
        component_name: component_name.to_owned(),
        display_name: "Loading semantic context...".to_owned(),
        ..ComponentMetadata::default()
    });

    spawn_local(async move {
        let metadata = match Request::get(&format!("/api/v1/components/{component_name}"))
            .send()
            .await
        {
            Ok(response) if response.ok() => response
                .json::<ComponentMetadata>()
                .await
                .unwrap_or_else(|_| ComponentMetadata {
                    component_name: component_name.to_owned(),
                    display_name: "Metadata response could not be parsed".to_owned(),
                    ..ComponentMetadata::default()
                }),
            _ => ComponentMetadata {
                component_name: component_name.to_owned(),
                display_name: "Semantic metadata unavailable".to_owned(),
                ..ComponentMetadata::default()
            },
        };
        set_metadata.set(metadata);

        if let Ok(response) = Request::get(&format!("/api/v1/components/{component_name}/related"))
            .send()
            .await
        {
            if response.ok() {
                if let Ok(related) = response.json::<Vec<String>>().await {
                    set_related_highlights(element_id, &related);
                    set_related_elements.set(related);
                }
            }
        }
    });
}

fn assemble_rover(
    set_exploded: WriteSignal<bool>,
    components: ReadSignal<Vec<StructureBinding>>,
    selected_element: ReadSignal<Option<String>>,
    set_selected_element: WriteSignal<Option<String>>,
    related_elements: ReadSignal<Vec<String>>,
    set_related_elements: WriteSignal<Vec<String>>,
) {
    set_semantic_selection(
        selected_element.get_untracked().as_deref(),
        &related_elements.get_untracked(),
        None,
    );
    set_selected_element.set(None);
    set_related_elements.set(vec![]);
    set_exploded.set(false);
    animate_components(&components.get_untracked(), false);
}

fn component_breadcrumb(component_name: &str) -> Vec<String> {
    let parent = match component_name {
        "WheelFrontLeft" | "WheelMiddleLeft" | "WheelRearLeft" => Some("LeftSuspension"),
        "WheelFrontRight" | "WheelMiddleRight" | "WheelRearRight" => Some("RightSuspension"),
        _ => None,
    };

    ["SojournerRover", parent.unwrap_or_default(), component_name]
        .into_iter()
        .filter(|segment| !segment.is_empty())
        .map(str::to_owned)
        .collect()
}

#[component]
pub fn App() -> impl IntoView {
    let (components, set_components) = create_signal(Vec::<StructureBinding>::new());
    let (load_error, set_load_error) = create_signal(None::<String>);
    let (is_exploded, set_exploded) = create_signal(false);
    let (selected_element, set_selected_element) = create_signal(None::<String>);
    let (metadata, set_metadata) = create_signal(ComponentMetadata::default());
    let (related_elements, set_related_elements) = create_signal(Vec::<String>::new());

    let structure = create_local_resource(|| (), |_| fetch_structure());
    create_effect(move |_| {
        if let Some(result) = structure.get() {
            match result {
                Ok(load) => {
                    set_components.set(load.bindings);
                    set_load_error.set(load.notice);
                }
                Err(error) => set_load_error.set(Some(error)),
            }
        }
    });

    view! {
        <main>
            <div
                style="position: fixed; z-index: 10; top: 1rem; left: 1rem; width: min(20rem, calc(100vw - 2rem)); box-sizing: border-box; padding: 0.85rem 1rem; color: #17324d; background: rgba(255, 255, 255, 0.82); border: 1px solid rgba(23, 50, 77, 0.15); border-radius: 0.5rem; font-family: monospace; font-size: 0.8rem; line-height: 1.5; box-shadow: 0 0.5rem 2rem rgba(23, 50, 77, 0.12);"
            >
                <strong style="display: block; font-size: 1.2rem;">"SOJOURNER ROVER"</strong>
                <p style="margin: 0.35rem 0 0;">"Click the rover to toggle the exploded graph view."</p>
                {move || load_error.get().map(|error| view! {
                    <p style="margin: 0.35rem 0 0;">{error}</p>
                })}
            </div>
            <div
                style="position: fixed; z-index: 10; top: 1rem; right: 1rem; width: min(15rem, calc(100vw - 2rem)); box-sizing: border-box; padding: 0.85rem 1rem; color: #17324d; background: rgba(255, 255, 255, 0.82); border: 1px solid rgba(23, 50, 77, 0.15); border-radius: 0.5rem; font-family: monospace; font-size: 0.8rem; line-height: 1.5; box-shadow: 0 0.5rem 2rem rgba(23, 50, 77, 0.12);"
            >
                <strong style="display: block; margin-bottom: 0.35rem; font-size: 0.9rem;">"NAVIGATION"</strong>
                <div>"W A S D  Move"</div>
                <div>"E / C  Up / Down"</div>
                <div>"Left Mouse  Look"</div>
                <div>"Scroll  Zoom"</div>
                <div>"Middle Mouse drag  Pan"</div>
            </div>
            <div
                style="position: fixed; z-index: 11; right: 1rem; bottom: 1rem; display: flex; align-items: flex-end; flex-direction: column; gap: 0.35rem; font-family: monospace; font-size: 0.8rem;"
            >
                <button
                    id="enter-vr-button"
                    type="button"
                    disabled=true
                    aria-label="Enter VR"
                    title="Enter VR"
                    style="display: grid; place-items: center; width: 3rem; height: 3rem; padding: 0; border: 1px solid #17324d; border-radius: 0.5rem; color: #ffffff; background: #17324d; font: inherit; cursor: pointer;"
                >
                    <svg
                        aria-hidden="true"
                        viewBox="0 0 24 24"
                        width="26"
                        height="26"
                        fill="currentColor"
                    >
                        <path
                            fill-rule="evenodd"
                            d="M4 5.5h16a2 2 0 0 1 2 2v8.25a2 2 0 0 1-2 2h-3.1a2 2 0 0 1-1.42-.59l-2.07-2.07a2 2 0 0 0-2.82 0l-2.07 2.07a2 2 0 0 1-1.42.59H4a2 2 0 0 1-2-2V7.5a2 2 0 0 1 2-2Zm4.25 8a2.75 2.75 0 1 0 0-5.5 2.75 2.75 0 0 0 0 5.5Zm7.5 0a2.75 2.75 0 1 0 0-5.5 2.75 2.75 0 0 0 0 5.5Z"
                        ></path>
                    </svg>
                </button>
                <span id="vr-status" style="color: #17324d; font-size: 0.75rem;">
                    "Checking WebXR..."
                </span>
            </div>
            <Show when=move || is_exploded.get()>
                <button
                    id="assemble-rover-button"
                    type="button"
                    on:click=move |_| {
                        assemble_rover(
                            set_exploded,
                            components,
                            selected_element,
                            set_selected_element,
                            related_elements,
                            set_related_elements,
                        )
                    }
                    style="position: fixed; z-index: 11; left: 50%; bottom: 1rem; transform: translateX(-50%); min-width: 10rem; padding: 0.75rem 1.1rem; border: 1px solid #17324d; border-radius: 0.5rem; color: #ffffff; background: #17324d; font-family: monospace; font-size: 0.8rem; font-weight: bold; cursor: pointer; box-shadow: 0 0.5rem 2rem rgba(23, 50, 77, 0.18);"
                >
                    "Assemble Rover"
                </button>
            </Show>
            <Show when=move || selected_element.get().is_some()>
                <div
                    id="selection-panel-stack"
                    style="position: fixed; z-index: 12; left: 1rem; top: 8.5rem; width: min(20rem, calc(100vw - 2rem)); max-height: calc(100vh - 9.5rem); display: flex; flex-direction: column; gap: 0.75rem; overflow-y: auto; padding-right: 0.25rem; box-sizing: border-box;"
                >
                <aside
                    id="semantic-panel"
                    style="position: relative; flex: none; width: 100%; box-sizing: border-box; padding: 0.85rem 1rem; color: #e6fff0; background: rgba(7, 27, 24, 0.94); border: 1px solid rgba(0, 255, 102, 0.45); border-radius: 0.5rem; font-family: monospace; font-size: 0.8rem; line-height: 1.5; box-shadow: 0 0.75rem 2.5rem rgba(7, 27, 24, 0.3); backdrop-filter: blur(0.5rem);"
                >
                    <button
                        id="close-semantic-panel"
                        type="button"
                        aria-label="Close semantic context"
                        on:click=move |_| {
                            set_semantic_selection(
                                selected_element.get_untracked().as_deref(),
                                &related_elements.get_untracked(),
                                None,
                            );
                            set_selected_element.set(None);
                            set_related_elements.set(vec![]);
                        }
                        style="position: absolute; top: 0.65rem; right: 0.65rem; width: 1.75rem; height: 1.75rem; padding: 0; color: #80ffb0; background: transparent; border: 1px solid rgba(128, 255, 176, 0.45); border-radius: 0.3rem; font: inherit; cursor: pointer;"
                    >
                        "X"
                    </button>
                    <div style="padding-right: 2.2rem;">
                        <strong
                            id="semantic-panel-title"
                            style="display: block; color: #00ff66; font-size: 1rem; line-height: 1.4;"
                        >
                            {move || metadata.get().display_name}
                        </strong>
                        <div
                            id="semantic-panel-id"
                            style="margin-top: 0.25rem; color: #80ffb0; font-size: 0.75rem;"
                        >
                            {move || format!("ID  {}", metadata.get().component_name)}
                        </div>
                    </div>
                    <div style="height: 1px; margin: 0.8rem 0; background: rgba(0, 255, 102, 0.3);"></div>
                    <div id="semantic-panel-details" style="display: grid; gap: 0.65rem; font-size: 0.78rem; line-height: 1.45;">
                        {move || metadata.get().category.map(|value| view! {
                            <div><strong style="color: #80ffb0;">"TYPE  "</strong>{value}</div>
                        })}
                        {move || metadata.get().purpose.map(|value| view! {
                            <div><strong style="color: #80ffb0;">"ROLE  "</strong>{value}</div>
                        })}
                        {move || metadata.get().power_requirement.map(|value| view! {
                            <div><strong style="color: #80ffb0;">"POWER "</strong>{value}</div>
                        })}
                        {move || metadata.get().mission_note.map(|value| view! {
                            <div><strong style="color: #80ffb0;">"LOG   "</strong>{value}</div>
                        })}
                    </div>
                </aside>
                <aside
                    id="oxigraph-trace"
                    style="position: relative; flex: none; width: 100%; box-sizing: border-box; padding: 0.9rem 1rem; color: rgba(23, 50, 77, 0.88); background: rgba(239, 255, 246, 0.94); border: 1px solid rgba(0, 166, 81, 0.2); border-radius: 0.5rem; font-family: monospace; font-size: 0.85rem; line-height: 1.55; box-shadow: 0 0.5rem 2rem rgba(23, 50, 77, 0.14); backdrop-filter: blur(0.4rem); pointer-events: none;"
                >
                <div style="display: flex; align-items: center; justify-content: space-between; gap: 0.75rem; margin-bottom: 0.45rem;">
                    <strong style="color: #17324d; font-size: 0.9rem; letter-spacing: 0.08em;">
                        "OXIGRAPH"
                    </strong>
                    <span style="color: rgba(23, 50, 77, 0.68); font-size: 0.72rem;">"SPARQL RESULT GRAPH"</span>
                </div>
                <div
                    id="oxigraph-breadcrumb"
                    style="display: flex; flex-wrap: wrap; gap: 0.25rem; margin-bottom: 0.45rem; color: rgba(23, 50, 77, 0.72);"
                >
                    {move || {
                        let component_name = selected_element
                            .get()
                            .map(|_| metadata.get().component_name)
                            .unwrap_or_default();
                        if component_name.is_empty() {
                            view! { <span>"SojournerRover"</span> }.into_view()
                        } else {
                            component_breadcrumb(&component_name)
                                .into_iter()
                                .enumerate()
                                .map(|(index, segment)| view! {
                                    {if index > 0 {
                                        view! { <span style="opacity: 0.45;">"›"</span> }.into_view()
                                    } else {
                                        ().into_view()
                                    }}
                                    <span>{segment}</span>
                                })
                                .collect_view()
                                .into_view()
                        }
                    }}
                </div>
                <code
                    id="oxigraph-query"
                    style="display: block; margin-bottom: 0.45rem; color: rgba(23, 50, 77, 0.66); white-space: normal;"
                >
                    {move || {
                        let component_name = selected_element
                            .get()
                            .map(|_| metadata.get().component_name)
                            .unwrap_or_default();
                        if component_name.is_empty() {
                            "SELECT ?part ?x ?y ?z WHERE { SojournerRover hasPart ?part . ?part offsetX ?x ; offsetY ?y ; offsetZ ?z . }".to_owned()
                        } else {
                            format!("SELECT ?predicate ?value WHERE {{ {component_name} ?predicate ?value . }}")
                        }
                    }}
                </code>
                <div id="oxigraph-relationships" style="display: grid; gap: 0.18rem;">
                    {move || {
                        let selected_metadata = metadata.get();
                        let component_name = selected_element
                            .get()
                            .map(|_| selected_metadata.component_name.clone())
                            .unwrap_or_default();
                        let related = related_elements.get();
                        if component_name.is_empty() {
                            view! {
                                <span>"SojournerRover —hasPart→ "{components.get().len()}" nodes"</span>
                            }.into_view()
                        } else {
                            let mut triples = Vec::new();
                            if let Some(category) = selected_metadata.category {
                                triples.push(format!("{component_name} —category→ {category}"));
                            }
                            if let Some(power) = selected_metadata.power_requirement {
                                triples.push(format!("{component_name} —powerRequirement→ {power}"));
                            }
                            triples.extend(
                                related
                                .into_iter()
                                .map(|related_name| format!("{component_name} —connectedTo→ {related_name}")),
                            );
                            if triples.is_empty() {
                                triples.push(format!("{component_name} —querying→ relationships"));
                            }
                            triples
                                .into_iter()
                                .map(|triple| view! { <span>{triple}</span> })
                                .collect_view()
                                .into_view()
                        }
                    }}
                </div>
                </aside>
                </div>
            </Show>

            <a-scene
                id="rover-scene"
                background="color: #e8edf2"
                cursor="rayOrigin: mouse"
                raycaster="objects: .clickable"
                renderer="colorManagement: true; physicallyCorrectLights: true; exposure: 1.15"
                webxr="requiredFeatures: local-floor; optionalFeatures: bounded-floor, hand-tracking; referenceSpaceType: local-floor"
                vr-mode-ui="enabled: false"
                webxr-launcher
            >
                <a-entity
                    id="assembled-rover"
                    class="clickable"
                    position="0 -1.1 0"
                    rotation="0 -25 0"
                    scale="2.4 2.4 2.4"
                    gltf-model="url(/public/models/sojourner-rover.glb)"
                    shadow="cast: true; receive: true"
                    on:click=move |_| explode_rover(is_exploded, set_exploded, components)
                ></a-entity>
                <a-entity
                    id="graph-parts"
                    position="0 -0.2 0"
                    rotation="0 -25 0"
                    visible="false"
                >
                    <a-entity
                        id="ExplodedBody"
                        data-component-name="BodyAssembly"
                        class="clickable"
                        semantic-highlight="state: none"
                        position="0 -0.9 0"
                        scale="0.12 0.12 0.12"
                        gltf-model="url(/public/models/rover-body.glb)"
                        shadow="cast: true; receive: true"
                        on:click=move |_| {
                            select_component(
                                "BodyAssembly",
                                "ExplodedBody",
                                selected_element,
                                set_selected_element,
                                set_metadata,
                                related_elements,
                                set_related_elements,
                            )
                        }
                    ></a-entity>
                    <a-entity
                        id="ExplodedAntenna"
                        data-component-name="Antenna"
                        class="clickable"
                        semantic-highlight="state: none"
                        position="0 -0.9 0"
                        scale="0.12 0.12 0.12"
                        gltf-model="url(/public/models/rover-antenna.glb)"
                        shadow="cast: true; receive: true"
                        on:click=move |_| {
                            select_component(
                                "Antenna",
                                "ExplodedAntenna",
                                selected_element,
                                set_selected_element,
                                set_metadata,
                                related_elements,
                                set_related_elements,
                            )
                        }
                    ></a-entity>
                    <a-entity id="ExplodedRightWheelAssembly" position="0 0 0">
                        <a-entity
                            id="ExplodedRightSuspension"
                            data-component-name="RightSuspension"
                            class="clickable"
                            semantic-highlight="state: none"
                            position="0 -0.9 0.9"
                            scale="0.1 0.1 0.1"
                            gltf-model="url(/public/models/rover-suspension-netfabb.glb)"
                            shadow="cast: true; receive: true"
                            on:click=move |_| {
                                select_component(
                                    "RightSuspension",
                                    "ExplodedRightSuspension",
                                    selected_element,
                                    set_selected_element,
                                    set_metadata,
                                    related_elements,
                                    set_related_elements,
                                )
                            }
                        ></a-entity>
                        {["ExplodedWheelRR", "ExplodedWheelMR", "ExplodedWheelFR"]
                            .into_iter()
                            .zip(["-1.4 -1 1.5", "0 -1 1.5", "1.4 -1 1.5"])
                            .zip(["WheelRearRight", "WheelMiddleRight", "WheelFrontRight"])
                            .map(|((id, position), component_name)| view! {
                            <a-entity
                                id=id
                                data-component-name=component_name
                                class="clickable"
                                semantic-highlight="state: none"
                                position=position
                                scale="2.4 2.4 2.4"
                                gltf-model="url(/public/models/rover-wheel.glb)"
                                shadow="cast: true; receive: true"
                                on:click=move |_| {
                                    select_component(
                                        component_name,
                                        id,
                                        selected_element,
                                        set_selected_element,
                                        set_metadata,
                                        related_elements,
                                        set_related_elements,
                                    )
                                }
                            ></a-entity>
                        })
                        .collect_view()}
                    </a-entity>
                    <a-entity id="ExplodedLeftWheelAssembly" position="0 0 0">
                        <a-entity
                            id="ExplodedLeftSuspension"
                            data-component-name="LeftSuspension"
                            class="clickable"
                            semantic-highlight="state: none"
                            position="0 -0.9 -0.9"
                            scale="0.1 0.1 -0.1"
                            gltf-model="url(/public/models/rover-suspension-netfabb.glb)"
                            shadow="cast: true; receive: true"
                            on:click=move |_| {
                                select_component(
                                    "LeftSuspension",
                                    "ExplodedLeftSuspension",
                                    selected_element,
                                    set_selected_element,
                                    set_metadata,
                                    related_elements,
                                    set_related_elements,
                                )
                            }
                        ></a-entity>
                        {["ExplodedWheelRL", "ExplodedWheelML", "ExplodedWheelFL"]
                            .into_iter()
                            .zip(["-1.4 -1 -1.5", "0 -1 -1.5", "1.4 -1 -1.5"])
                            .zip(["WheelRearLeft", "WheelMiddleLeft", "WheelFrontLeft"])
                            .map(|((id, position), component_name)| view! {
                            <a-entity
                                id=id
                                data-component-name=component_name
                                class="clickable"
                                semantic-highlight="state: none"
                                position=position
                                scale="2.4 2.4 -2.4"
                                gltf-model="url(/public/models/rover-wheel.glb)"
                                shadow="cast: true; receive: true"
                                on:click=move |_| {
                                    select_component(
                                        component_name,
                                        id,
                                        selected_element,
                                        set_selected_element,
                                        set_metadata,
                                        related_elements,
                                        set_related_elements,
                                    )
                                }
                            ></a-entity>
                        })
                        .collect_view()}
                    </a-entity>
                    <a-entity
                        id="ExplodedDrill"
                        data-component-name="Drill"
                        class="clickable"
                        semantic-highlight="state: none"
                        position="0 -0.4 0"
                        scale="2.4 2.4 2.4"
                        gltf-model="url(/public/models/rover-drill.glb)"
                        shadow="cast: true; receive: true"
                        on:click=move |_| {
                            select_component(
                                "Drill",
                                "ExplodedDrill",
                                selected_element,
                                set_selected_element,
                                set_metadata,
                                related_elements,
                                set_related_elements,
                            )
                        }
                    ></a-entity>
                </a-entity>

                <a-plane
                    class="clickable"
                    position="0 -1.5 0"
                    rotation="-90 0 0"
                    width="40"
                    height="40"
                    material="color: #f7f8fa; roughness: 0.95; metalness: 0"
                    shadow="receive: true"
                ></a-plane>
                <a-sky class="clickable" color="#e8edf2"></a-sky>
                <a-light type="ambient" color="#dce8f5" intensity="1.15"></a-light>
                <a-light
                    type="directional"
                    color="#fff4df"
                    intensity="2.4"
                    position="-4 7 5"
                    shadow="cast: true"
                ></a-light>
                <a-light
                    type="directional"
                    color="#d9eaff"
                    intensity="1.5"
                    position="5 4 2"
                ></a-light>
                <a-light
                    type="point"
                    color="#ffffff"
                    intensity="1.1"
                    distance="18"
                    position="0 5 -5"
                ></a-light>
                <a-entity id="camera-rig" position="5.5 1.65 -3.5" vr-locomotion>
                    <a-camera
                        initial-camera-look="pitch: -15; yaw: 120"
                        camera="fov: 62"
                        look-controls="pointerLockEnabled: false"
                        wasd-controls="acceleration: 25"
                        vertical-controls="speed: 3"
                        viewport-controls="zoomSpeed: 0.0025; panSpeed: 0.004"
                    >
                        <a-entity
                            id="vr-assemble-control"
                            class="clickable"
                            vr-assemble-control=move || format!("active: {}", is_exploded.get())
                            always-on-top
                            position="0.7 -0.55 -2.4"
                            on:click=move |_| {
                                assemble_rover(
                                    set_exploded,
                                    components,
                                    selected_element,
                                    set_selected_element,
                                    related_elements,
                                    set_related_elements,
                                )
                            }
                        >
                            <a-plane
                                width="0.48"
                                height="0.14"
                                material="color: #17324d; opacity: 0.9; transparent: true; depthTest: false"
                            ></a-plane>
                            <a-text
                                position="0 0 0.01"
                                align="center"
                                width="0.42"
                                color="#ffffff"
                                value="ASSEMBLE"
                            ></a-text>
                        </a-entity>
                    </a-camera>
                    <a-entity
                        id="left-controller"
                        laser-controls="hand: left"
                        raycaster="objects: .clickable"
                    ></a-entity>
                    <a-entity
                        id="right-controller"
                        laser-controls="hand: right"
                        raycaster="objects: .clickable"
                    ></a-entity>
                </a-entity>
            </a-scene>
        </main>
    }
}
