use gloo_net::http::Request;
use leptos::*;
use serde::Deserialize;
use web_sys::window;

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
}

async fn fetch_structure() -> Result<Vec<StructureBinding>, String> {
    Request::get("/api/v1/structure")
        .send()
        .await
        .map_err(|error| format!("Gateway request failed: {error}"))?
        .json::<Vec<StructureBinding>>()
        .await
        .map_err(|error| format!("Invalid gateway response: {error}"))
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
            "0 0 0".to_owned()
        };
        let animation =
            format!("property: position; to: {target}; dur: 1000; easing: easeOutElastic;");
        let _ = element.set_attribute("animation__position", &animation);
    }
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
                Ok(bindings) => set_components.set(bindings),
                Err(error) => set_load_error.set(Some(error)),
            }
        }
    });

    let toggle_explosion = move |_| {
        let exploded = !is_exploded.get_untracked();
        set_exploded.set(exploded);
        animate_components(&components.get_untracked(), exploded);
    };

    view! {
        <main>
            <div
                style="position: fixed; z-index: 10; top: 1rem; left: 1rem; color: #00ff33; font-family: monospace;"
            >
                <strong>"CADMUS // SOJOURNER ROVER"</strong>
                <p>"Click the rover to toggle the exploded graph view."</p>
                {move || load_error.get().map(|error| view! { <p>{error}</p> })}
            </div>

            <a-scene background="color: #000000">
                <a-entity id="rover-group" class="clickable" on:click=toggle_explosion>
                    <a-box
                        id="ChassisBase"
                        position="0 0 0"
                        width="2.8"
                        height="0.8"
                        depth="1.8"
                        material="wireframe: true; color: #00FF33; emissive: #00FF33"
                    ></a-box>
                    <a-box
                        id="CameraMast"
                        position="0 0 0"
                        width="0.3"
                        height="1.4"
                        depth="0.3"
                        material="wireframe: true; color: #00FF33; emissive: #00FF33"
                    ></a-box>
                    <a-cylinder
                        id="LeftWheel"
                        position="0 0 0"
                        rotation="0 0 90"
                        radius="0.55"
                        height="0.35"
                        material="wireframe: true; color: #00FF33; emissive: #00FF33"
                    ></a-cylinder>
                    <a-cylinder
                        id="RightWheel"
                        position="0 0 0"
                        rotation="0 0 90"
                        radius="0.55"
                        height="0.35"
                        material="wireframe: true; color: #00FF33; emissive: #00FF33"
                    ></a-cylinder>
                </a-entity>

                <a-plane
                    position="0 -1.2 -2"
                    rotation="-90 0 0"
                    width="30"
                    height="30"
                    material="wireframe: true; color: #003311"
                ></a-plane>
                <a-sky color="#000000"></a-sky>
                <a-camera position="0 2.5 8"></a-camera>
            </a-scene>
        </main>
    }
}
