import fs from "node:fs";

const [inputPath, outputPath, referencePath = inputPath] = process.argv.slice(2);
if (!inputPath || !outputPath) {
  throw new Error(
    "Usage: node scripts/stl-to-glb.mjs input.stl output.glb [reference.stl]",
  );
}

const stl = fs.readFileSync(inputPath);
const triangleCount = stl.readUInt32LE(80);
if (stl.length !== 84 + triangleCount * 50) {
  throw new Error("Only binary STL files are supported");
}

const reference = fs.readFileSync(referencePath);
const referenceTriangleCount = reference.readUInt32LE(80);
const sourceMin = [Infinity, Infinity, Infinity];
const sourceMax = [-Infinity, -Infinity, -Infinity];
for (let triangle = 0; triangle < referenceTriangleCount; triangle += 1) {
  const source = 84 + triangle * 50;
  for (let vertex = 0; vertex < 3; vertex += 1) {
    const sourceVertex = source + 12 + vertex * 12;
    for (let axis = 0; axis < 3; axis += 1) {
      const value = reference.readFloatLE(sourceVertex + axis * 4);
      sourceMin[axis] = Math.min(sourceMin[axis], value);
      sourceMax[axis] = Math.max(sourceMax[axis], value);
    }
  }
}

const sourceCenter = [
  (sourceMin[0] + sourceMax[0]) / 2,
  (sourceMin[1] + sourceMax[1]) / 2,
];
const positions = Buffer.alloc(triangleCount * 9 * 4);
const normals = Buffer.alloc(triangleCount * 9 * 4);
const min = [Infinity, Infinity, Infinity];
const max = [-Infinity, -Infinity, -Infinity];

for (let triangle = 0; triangle < triangleCount; triangle += 1) {
  const source = 84 + triangle * 50;
  const normal = [
    stl.readFloatLE(source),
    stl.readFloatLE(source + 8),
    -stl.readFloatLE(source + 4),
  ];

  for (let vertex = 0; vertex < 3; vertex += 1) {
    const sourceVertex = source + 12 + vertex * 12;
    const targetVertex = (triangle * 3 + vertex) * 12;

    const sourcePosition = [
      stl.readFloatLE(sourceVertex) - sourceCenter[0],
      stl.readFloatLE(sourceVertex + 8) - sourceMin[2],
      -(stl.readFloatLE(sourceVertex + 4) - sourceCenter[1]),
    ];

    for (let axis = 0; axis < 3; axis += 1) {
      const value = sourcePosition[axis];
      positions.writeFloatLE(value, targetVertex + axis * 4);
      normals.writeFloatLE(normal[axis], targetVertex + axis * 4);
      min[axis] = Math.min(min[axis], value);
      max[axis] = Math.max(max[axis], value);
    }
  }
}

const binary = Buffer.concat([positions, normals]);
const gltf = {
  asset: { version: "2.0", generator: "Cadmus STL to GLB converter" },
  scene: 0,
  scenes: [{ nodes: [0] }],
  nodes: [{ mesh: 0 }],
  meshes: [
    {
      primitives: [
        {
          attributes: { POSITION: 0, NORMAL: 1 },
          material: 0,
          mode: 4,
        },
      ],
    },
  ],
  materials: [
    {
      name: "Sojourner Gold",
      pbrMetallicRoughness: {
        baseColorFactor: [0.65, 0.36, 0.08, 1],
        metallicFactor: 0.65,
        roughnessFactor: 0.45,
      },
    },
  ],
  buffers: [{ byteLength: binary.length }],
  bufferViews: [
    { buffer: 0, byteOffset: 0, byteLength: positions.length, target: 34962 },
    {
      buffer: 0,
      byteOffset: positions.length,
      byteLength: normals.length,
      target: 34962,
    },
  ],
  accessors: [
    {
      bufferView: 0,
      componentType: 5126,
      count: triangleCount * 3,
      type: "VEC3",
      min,
      max,
    },
    {
      bufferView: 1,
      componentType: 5126,
      count: triangleCount * 3,
      type: "VEC3",
    },
  ],
};

const json = Buffer.from(JSON.stringify(gltf));
const jsonPadding = (4 - (json.length % 4)) % 4;
const binaryPadding = (4 - (binary.length % 4)) % 4;
const paddedJson = Buffer.concat([json, Buffer.alloc(jsonPadding, 0x20)]);
const paddedBinary = Buffer.concat([binary, Buffer.alloc(binaryPadding)]);

const header = Buffer.alloc(12);
header.writeUInt32LE(0x46546c67, 0);
header.writeUInt32LE(2, 4);
header.writeUInt32LE(12 + 8 + paddedJson.length + 8 + paddedBinary.length, 8);

const jsonHeader = Buffer.alloc(8);
jsonHeader.writeUInt32LE(paddedJson.length, 0);
jsonHeader.writeUInt32LE(0x4e4f534a, 4);

const binaryHeader = Buffer.alloc(8);
binaryHeader.writeUInt32LE(paddedBinary.length, 0);
binaryHeader.writeUInt32LE(0x004e4942, 4);

fs.writeFileSync(
  outputPath,
  Buffer.concat([header, jsonHeader, paddedJson, binaryHeader, paddedBinary]),
);

console.log(
  `Converted ${triangleCount} triangles; bounds ${JSON.stringify({ min, max })}`,
);
