import { albumTools } from "./build/albums.js";
import { playTools } from "./build/play.js";
import { readTools } from "./build/read.js";

const [,, toolName, rawArgs] = process.argv;

if (!toolName) {
  console.error("Missing tool name.");
  process.exit(1);
}

const tools = [...readTools, ...playTools, ...albumTools];
const tool = tools.find((entry) => entry.name === toolName);

if (!tool) {
  console.error(`Unknown tool: ${toolName}`);
  process.exit(1);
}

let args = {};
if (rawArgs) {
  try {
    args = JSON.parse(rawArgs);
  } catch (error) {
    console.error(`Invalid JSON args: ${error instanceof Error ? error.message : String(error)}`);
    process.exit(1);
  }
}

try {
  const result = await tool.handler(args, {});
  process.stdout.write(JSON.stringify(result));
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stdout.write(JSON.stringify({ content: [{ type: "text", text: message, isError: true }] }));
  process.exit(1);
}
