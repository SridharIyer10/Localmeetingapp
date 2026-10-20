import { defineConfig } from "vitest/config";

export default defineConfig({
  resolve: {
    // Logic tests were written against bun:test's describe/test/expect.
    alias: { "bun:test": "vitest" },
  },
  test: {
    include: ["src/**/*.test.ts", "tests/lib/**/*.test.ts"],
  },
});
