import fs from "node:fs";

const source = fs.readFileSync("public/models/rover-body-netfabb.stl");
const triangleCount = source.readUInt32LE(80);
const parents = Array.from({ length: triangleCount }, (_, index) => index);
const vertexOwners = new Map();
const maximumZ = Array(triangleCount).fill(-Infinity);
const body = [];
const antenna = [];

function find(index) {
  if (parents[index] !== index) {
    parents[index] = find(parents[index]);
  }
  return parents[index];
}

function union(left, right) {
  const leftRoot = find(left);
  const rightRoot = find(right);
  if (leftRoot !== rightRoot) {
    parents[rightRoot] = leftRoot;
  }
}

for (let triangle = 0; triangle < triangleCount; triangle += 1) {
  for (let vertex = 0; vertex < 3; vertex += 1) {
    const offset = 84 + triangle * 50 + 12 + vertex * 12;
    const coordinates = [0, 1, 2].map((axis) =>
      source.readFloatLE(offset + axis * 4),
    );
    const key = coordinates.join(",");
    maximumZ[triangle] = Math.max(maximumZ[triangle], coordinates[2]);
    if (vertexOwners.has(key)) {
      union(triangle, vertexOwners.get(key));
    } else {
      vertexOwners.set(key, triangle);
    }
  }
}

const componentMaximumZ = new Map();
for (let triangle = 0; triangle < triangleCount; triangle += 1) {
  const root = find(triangle);
  componentMaximumZ.set(
    root,
    Math.max(componentMaximumZ.get(root) ?? -Infinity, maximumZ[triangle]),
  );
}

for (let triangle = 0; triangle < triangleCount; triangle += 1) {
  (componentMaximumZ.get(find(triangle)) > 15 ? antenna : body).push(triangle);
}

for (const [name, triangles] of Object.entries({ body, antenna })) {
  const output = Buffer.alloc(84 + triangles.length * 50);
  source.copy(output, 0, 0, 80);
  output.writeUInt32LE(triangles.length, 80);
  triangles.forEach((triangle, index) => {
    source.copy(
      output,
      84 + index * 50,
      84 + triangle * 50,
      84 + triangle * 50 + 50,
    );
  });
  fs.writeFileSync(`public/models/rover-${name}.stl`, output);
}
