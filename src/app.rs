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

fn toggle_explosion(
    is_exploded: ReadSignal<bool>,
    set_exploded: WriteSignal<bool>,
    components: ReadSignal<Vec<StructureBinding>>,
) {
    let exploded = !is_exploded.get_untracked();
    set_exploded.set(exploded);
    animate_components(&components.get_untracked(), exploded);
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
        if let Ok(Some(related)) = document.query_selector(&format!("[data-component-name='{}']", related_name)) {
            let _ = related.set_attribute("semantic-highlight", "state: none");
            let related_id = related.id();
            if let Some(tether) = document.get_element_by_id(&format!("tether-{related_id}")) {
                tether.remove();
            }
        }
    }

    let Some(element_id) = element_id else {
        if let Some(hud) = document.get_element_by_id("semantic-hud") {
            let _ = hud.set_attribute("visible", "false");
        }
        if let Some(tether) = document.get_element_by_id("semantic-tether") {
            let _ = tether.set_attribute("visible", "false");
        }
        return;
    };

    if let Some(selected) = document.get_element_by_id(element_id) {
        let _ = selected.set_attribute("semantic-highlight", "state: active");
    }
    if let Some(hud) = document.get_element_by_id("semantic-hud") {
        let _ = hud.set_attribute("semantic-hud", &format!("target: #{element_id}"));
        let _ = hud.set_attribute("visible", "true");
    }
    if let Some(tether) = document.get_element_by_id("semantic-tether") {
        let _ = tether.set_attribute(
            "relationship-tether",
            &format!("target: #{element_id}; card: #semantic-hud; color: #00ff66"),
        );
        let _ = tether.set_attribute("visible", "true");
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

        if let Ok(response) =
            Request::get(&format!("/api/v1/components/{component_name}/related"))
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

fn metadata_details(metadata: &ComponentMetadata) -> String {
    [
        metadata
            .category
            .as_ref()
            .map(|value| format!("TYPE  {value}")),
        metadata
            .purpose
            .as_ref()
            .map(|value| format!("ROLE  {value}")),
        metadata
            .power_requirement
            .as_ref()
            .map(|value| format!("POWER {value}")),
        metadata
            .mission_note
            .as_ref()
            .map(|value| format!("LOG   {value}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n\n")
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
                style="position: fixed; z-index: 10; top: 1rem; left: 1rem; max-width: 70%; padding: 0.75rem 1rem; color: #17324d; background: rgba(255, 255, 255, 0.82); border: 1px solid rgba(23, 50, 77, 0.15); border-radius: 0.5rem; font-family: monospace; box-shadow: 0 0.5rem 2rem rgba(23, 50, 77, 0.12);"
            >
                <strong>"SOJOURNER ROVER"</strong>
                <p>"Click the rover to toggle the exploded graph view."</p>
                {move || load_error.get().map(|error| view! { <p>{error}</p> })}
            </div>
            <div
                style="position: fixed; z-index: 10; top: 1rem; right: 1rem; padding: 0.75rem 1rem; color: #17324d; background: rgba(255, 255, 255, 0.82); border: 1px solid rgba(23, 50, 77, 0.15); border-radius: 0.5rem; font-family: monospace; text-align: right; line-height: 1.5; box-shadow: 0 0.5rem 2rem rgba(23, 50, 77, 0.12);"
            >
                <strong>"NAVIGATION"</strong>
                <div>"W A S D  Move"</div>
                <div>"E / C  Up / Down"</div>
                <div>"Left Mouse  Look"</div>
                <div>"Scroll  Zoom"</div>
                <div>"Middle Mouse drag  Pan"</div>
                <div>"Click rover  Explode"</div>
                <div>"Click part  Semantic context"</div>
            </div>
            <div
                style="position: fixed; z-index: 11; right: 1rem; bottom: 1rem; display: flex; align-items: flex-end; flex-direction: column; gap: 0.35rem; font-family: monospace;"
            >
                <button
                    id="assemble-rover-button"
                    type="button"
                    disabled=move || !is_exploded.get()
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
                    style="padding: 0.65rem 1rem; border: 1px solid #17324d; border-radius: 0.5rem; color: #17324d; background: rgba(255, 255, 255, 0.9); font: inherit; font-weight: bold; cursor: pointer;"
                >
                    "Assemble Rover"
                </button>
                <button
                    id="enter-vr-button"
                    type="button"
                    disabled=true
                    style="padding: 0.75rem 1.1rem; border: 1px solid #17324d; border-radius: 0.5rem; color: #ffffff; background: #17324d; font: inherit; font-weight: bold; cursor: pointer;"
                >
                    "Enter VR"
                </button>
                <span id="vr-status" style="color: #17324d; font-size: 0.75rem;">
                    "Checking WebXR..."
                </span>
            </div>

            <a-scene
                id="rover-scene"
                background="color: #e8edf2"
                cursor="rayOrigin: mouse"
                raycaster="objects: .clickable"
                renderer="colorManagement: true; physicallyCorrectLights: true; exposure: 1.15"
                webxr="optionalFeatures: local-floor, bounded-floor, hand-tracking"
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
                    on:click=move |_| toggle_explosion(is_exploded, set_exploded, components)
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
                                scale="5 5 5"
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
                                scale="5 5 -5"
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
                        scale="5 5 5"
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

                <a-entity id="semantic-tether" visible="false"></a-entity>
                <a-entity id="semantic-hud" visible="false" semantic-hud always-on-top>
                    <a-plane
                        width="1.8"
                        height="1.4"
                        material="color: #071b18; opacity: 0.92; transparent: true; side: double; depthTest: false"
                    ></a-plane>
                    <a-text
                        position="0 0.55 -0.02"
                        rotation="0 180 0"
                        align="center"
                        anchor="center"
                        baseline="top"
                        width="1.6"
                        wrap-count="26"
                        color="#00ff66"
                        value=move || metadata.get().display_name
                    ></a-text>
                    <a-text
                        position="0 0.35 -0.02"
                        rotation="0 180 0"
                        align="center"
                        anchor="center"
                        baseline="top"
                        width="1.6"
                        wrap-count="45"
                        color="#80ffb0"
                        value=move || format!("ID    {}", metadata.get().component_name)
                    ></a-text>
                    <a-plane
                        position="0 0.24 -0.02"
                        rotation="0 180 0"
                        width="1.5"
                        height="0.005"
                        material="color: #00ff66; opacity: 0.35; shader: flat; transparent: true; depthTest: false"
                    ></a-plane>
                    <a-text
                        position="0 0.15 -0.02"
                        rotation="0 180 0"
                        align="left"
                        anchor="center"
                        baseline="top"
                        width="1.5"
                        wrap-count="46"
                        color="#e6fff0"
                        value=move || metadata_details(&metadata.get())
                    ></a-text>
                </a-entity>

                <a-plane
                    class="clickable"
                    position="0 -1.1 0"
                    rotation="-90 0 0"
                    width="40"
                    height="40"
                    material="color: #f7f8fa; roughness: 0.95; metalness: 0"
                    shadow="receive: true"
                    on:click=move |_| {
                        set_semantic_selection(
                            selected_element.get_untracked().as_deref(),
                            &related_elements.get_untracked(),
                            None,
                        );
                        set_selected_element.set(None);
                        set_related_elements.set(vec![]);
                    }
                ></a-plane>
                <a-sky
                    class="clickable"
                    color="#e8edf2"
                    on:click=move |_| {
                        set_semantic_selection(
                            selected_element.get_untracked().as_deref(),
                            &related_elements.get_untracked(),
                            None,
                        );
                        set_selected_element.set(None);
                        set_related_elements.set(vec![]);
                    }
                ></a-sky>
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
                <a-entity
                    laser-controls="hand: left"
                    raycaster="objects: .clickable"
                ></a-entity>
                <a-entity
                    laser-controls="hand: right"
                    raycaster="objects: .clickable"
                ></a-entity>
                <a-camera
                    position="5.5 1.65 -3.5"
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
            </a-scene>
        </main>
    }
}
