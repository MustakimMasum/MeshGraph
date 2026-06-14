use gloo_net::http::Request;
use leptos::*;
use serde::Deserialize;
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
        ("LeftSuspension", "0.0", "-0.9", "2.2"),
        ("RightSuspension", "0.0", "-0.9", "-2.2"),
        ("WheelFrontLeft", "-1.4", "-1.0", "2.8"),
        ("WheelMiddleLeft", "0.0", "-1.0", "2.8"),
        ("WheelRearLeft", "1.4", "-1.0", "2.8"),
        ("WheelFrontRight", "-1.4", "-1.0", "-2.8"),
        ("WheelMiddleRight", "0.0", "-1.0", "-2.8"),
        ("WheelRearRight", "1.4", "-1.0", "-2.8"),
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
        ("ExplodedLeftWheelAssembly", "0 0 0", "0 0 1.3"),
        ("ExplodedRightWheelAssembly", "0 0 0", "0 0 -1.3"),
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

#[component]
pub fn App() -> impl IntoView {
    let (components, set_components) = create_signal(Vec::<StructureBinding>::new());
    let (load_error, set_load_error) = create_signal(None::<String>);
    let (is_exploded, set_exploded) = create_signal(false);

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
                <div>"Mouse  Look"</div>
                <div>"Scroll  Zoom"</div>
                <div>"Middle drag  Pan"</div>
                <div>"Click rover  Explode / Assemble"</div>
            </div>
            <div
                style="position: fixed; z-index: 11; right: 1rem; bottom: 1rem; display: flex; align-items: flex-end; flex-direction: column; gap: 0.35rem; font-family: monospace;"
            >
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
                        class="clickable"
                        position="0 -0.9 0"
                        scale="0.12 0.12 0.12"
                        gltf-model="url(/public/models/rover-body.glb)"
                        shadow="cast: true; receive: true"
                        on:click=move |_| toggle_explosion(is_exploded, set_exploded, components)
                    ></a-entity>
                    <a-entity
                        id="ExplodedAntenna"
                        class="clickable"
                        position="0 -0.9 0"
                        scale="0.12 0.12 0.12"
                        gltf-model="url(/public/models/rover-antenna.glb)"
                        shadow="cast: true; receive: true"
                        on:click=move |_| toggle_explosion(is_exploded, set_exploded, components)
                    ></a-entity>
                    <a-entity id="ExplodedLeftWheelAssembly" position="0 0 0">
                        <a-entity
                            id="ExplodedLeftSuspension"
                            class="clickable"
                            position="0 -0.9 0.9"
                            scale="0.1 0.1 0.1"
                            gltf-model="url(/public/models/rover-suspension-netfabb.glb)"
                            shadow="cast: true; receive: true"
                            on:click=move |_| toggle_explosion(is_exploded, set_exploded, components)
                        ></a-entity>
                        {["ExplodedWheelFL", "ExplodedWheelML", "ExplodedWheelRL"]
                            .into_iter()
                            .zip(["-1.4 -1 1.5", "0 -1 1.5", "1.4 -1 1.5"])
                            .map(|(id, position)| view! {
                            <a-entity
                                id=id
                                class="clickable"
                                position=position
                                scale="5 5 5"
                                gltf-model="url(/public/models/rover-wheel.glb)"
                                shadow="cast: true; receive: true"
                                on:click=move |_| toggle_explosion(is_exploded, set_exploded, components)
                            ></a-entity>
                        })
                        .collect_view()}
                    </a-entity>
                    <a-entity id="ExplodedRightWheelAssembly" position="0 0 0">
                        <a-entity
                            id="ExplodedRightSuspension"
                            class="clickable"
                            position="0 -0.9 -0.9"
                            scale="0.1 0.1 -0.1"
                            gltf-model="url(/public/models/rover-suspension-netfabb.glb)"
                            shadow="cast: true; receive: true"
                            on:click=move |_| toggle_explosion(is_exploded, set_exploded, components)
                        ></a-entity>
                        {["ExplodedWheelFR", "ExplodedWheelMR", "ExplodedWheelRR"]
                            .into_iter()
                            .zip(["-1.4 -1 -1.5", "0 -1 -1.5", "1.4 -1 -1.5"])
                            .map(|(id, position)| view! {
                            <a-entity
                                id=id
                                class="clickable"
                                position=position
                                scale="5 5 -5"
                                gltf-model="url(/public/models/rover-wheel.glb)"
                                shadow="cast: true; receive: true"
                                on:click=move |_| toggle_explosion(is_exploded, set_exploded, components)
                            ></a-entity>
                        })
                        .collect_view()}
                    </a-entity>
                    <a-entity
                        id="ExplodedDrill"
                        class="clickable"
                        position="0 -0.4 0"
                        scale="5 5 5"
                        gltf-model="url(/public/models/rover-drill.glb)"
                        shadow="cast: true; receive: true"
                        on:click=move |_| toggle_explosion(is_exploded, set_exploded, components)
                    ></a-entity>
                </a-entity>

                <a-plane
                    position="0 -1.1 0"
                    rotation="-90 0 0"
                    width="40"
                    height="40"
                    material="color: #f7f8fa; roughness: 0.95; metalness: 0"
                    shadow="receive: true"
                ></a-plane>
                <a-sky color="#e8edf2"></a-sky>
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
                    position="3.9 1.45 -2.2"
                    initial-camera-look="pitch: -15; yaw: 120"
                    camera="fov: 62"
                    look-controls="pointerLockEnabled: false"
                    wasd-controls="acceleration: 25"
                    vertical-controls="speed: 3"
                    viewport-controls="zoomSpeed: 0.0025; panSpeed: 0.004"
                ></a-camera>
            </a-scene>
        </main>
    }
}
