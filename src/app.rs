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

impl StructureBinding {
    fn position(&self) -> String {
        format!("{} {} {}", self.x.value, self.y.value, self.z.value)
    }

    fn assembled_position(&self) -> &'static str {
        match self.part_name.value.as_str() {
            "CameraMast" => "0 1.1 0",
            "LeftWheel" => "-1.4 -0.45 0",
            "RightWheel" => "1.4 -0.45 0",
            _ => "0 0 0",
        }
    }
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
        ("ChassisBase", "0.0", "0.0", "0.0"),
        ("CameraMast", "0.0", "1.8", "0.0"),
        ("LeftWheel", "-2.2", "-0.5", "0.0"),
        ("RightWheel", "2.2", "-0.5", "0.0"),
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

    for component in components {
        let Some(element) = document.get_element_by_id(&component.part_name.value) else {
            continue;
        };
        let target = if exploded {
            component.position()
        } else {
            component.assembled_position().to_owned()
        };
        let animation =
            format!("property: position; to: {target}; dur: 1000; easing: easeOutElastic;");
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
                style="position: fixed; z-index: 10; top: 1rem; left: 1rem; color: #00ff33; font-family: monospace;"
            >
                <strong>"CADMUS // SOJOURNER ROVER"</strong>
                <p>"Click the rover to toggle the exploded graph view."</p>
                {move || load_error.get().map(|error| view! { <p>{error}</p> })}
            </div>
            <div
                style="position: fixed; z-index: 10; top: 1rem; right: 1rem; color: #00ff33; font-family: monospace; text-align: right; line-height: 1.5;"
            >
                <strong>"NAVIGATION"</strong>
                <div>"W A S D  Move"</div>
                <div>"E / C  Up / Down"</div>
                <div>"Mouse  Look"</div>
                <div>"Click rover  Explode / Assemble"</div>
            </div>

            <a-scene background="color: #000000" cursor="rayOrigin: mouse">
                <a-entity
                    id="rover-group"
                    class="clickable"
                    position="0 0.4 0"
                    rotation="0 -25 0"
                >
                    <a-box
                        id="ChassisBase"
                        class="clickable"
                        position="0 0 0"
                        width="2.8"
                        height="0.8"
                        depth="1.8"
                        material="wireframe: true; color: #00FF33; emissive: #00FF33"
                        on:click=move |_| toggle_explosion(is_exploded, set_exploded, components)
                    ></a-box>
                    <a-box
                        id="CameraMast"
                        class="clickable"
                        position="0 1.1 0"
                        width="0.3"
                        height="1.4"
                        depth="0.3"
                        material="wireframe: true; color: #00FF33; emissive: #00FF33"
                        on:click=move |_| toggle_explosion(is_exploded, set_exploded, components)
                    ></a-box>
                    <a-cylinder
                        id="LeftWheel"
                        class="clickable"
                        position="-1.4 -0.45 0"
                        rotation="0 0 90"
                        radius="0.55"
                        height="0.35"
                        material="wireframe: true; color: #00FF33; emissive: #00FF33"
                        on:click=move |_| toggle_explosion(is_exploded, set_exploded, components)
                    ></a-cylinder>
                    <a-cylinder
                        id="RightWheel"
                        class="clickable"
                        position="1.4 -0.45 0"
                        rotation="0 0 90"
                        radius="0.55"
                        height="0.35"
                        material="wireframe: true; color: #00FF33; emissive: #00FF33"
                        on:click=move |_| toggle_explosion(is_exploded, set_exploded, components)
                    ></a-cylinder>
                </a-entity>

                <a-plane
                    position="0 -1.1 0"
                    rotation="-90 0 0"
                    width="30"
                    height="30"
                    material="wireframe: true; color: #003311"
                ></a-plane>
                <a-sky color="#000000"></a-sky>
                <a-camera
                    position="0 2.8 8"
                    rotation="-14 0 0"
                    look-controls="pointerLockEnabled: false"
                    wasd-controls="acceleration: 25"
                    vertical-controls="speed: 3"
                ></a-camera>
            </a-scene>
        </main>
    }
}
