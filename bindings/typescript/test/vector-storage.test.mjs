import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { vectorStoreSearchFiltered, vectorStoreUpsert } from "../dist/index.js";

for (const backend of ["lance_db", "qdrant"]) {
  test(
    `metadata filters agree in ${backend}`,
    {
      skip: backend === "qdrant" && !process.env.VECTORA_TEST_QDRANT_URL,
    },
    async () => {
      const dir =
        backend === "lance_db"
          ? await mkdtemp(join(tmpdir(), "engine-vector-filter-"))
          : null;
      const endpoint = dir ?? process.env.VECTORA_TEST_QDRANT_URL;
      const config = { backend, endpoint };
      const collection = `vector_filter_${randomUUID().replaceAll("-", "")}`;

      try {
        for (const [id, category, score] of [
          [1, "guide", 0.9],
          [2, "draft", 0.8],
          [3, "guide", 0.4],
          [4, ["guide"], 0.95],
        ]) {
          await vectorStoreUpsert(config, collection, {
            id,
            scope: "team",
            vector: [1, 0, 0],
            metadata: { category, score, nested: { language: "pt" } },
          });
        }
        await vectorStoreUpsert(config, collection, {
          id: 5,
          scope: "team",
          vector: [1, 0, 0],
          metadata: { score: 0.2 },
        });

        const hits = await vectorStoreSearchFiltered(
          config,
          collection,
          "team",
          [1, 0, 0],
          {
            op: "and",
            filters: [
              { op: "eq", path: "category", value: "guide" },
              { op: "gte", path: "score", value: 0.5 },
              { op: "eq", path: "nested.language", value: "pt" },
            ],
          },
        );
        assert.deepEqual(
          hits.map((hit) => hit.id),
          [1],
        );

        const excluded = await vectorStoreSearchFiltered(
          config,
          collection,
          "team",
          [1, 0, 0],
          {
            op: "not",
            filter: { op: "eq", path: "category", value: "guide" },
          },
        );
        assert.deepEqual(
          excluded.map((hit) => hit.id).sort((a, b) => a - b),
          [2, 4, 5],
        );

        const either = await vectorStoreSearchFiltered(
          config,
          collection,
          "team",
          [1, 0, 0],
          {
            op: "or",
            filters: [
              { op: "eq", path: "category", value: "draft" },
              { op: "in", path: "score", values: [0.4] },
            ],
          },
        );
        assert.deepEqual(
          either.map((hit) => hit.id).sort((a, b) => a - b),
          [2, 3],
        );
      } finally {
        if (dir) {
          await rm(dir, { recursive: true, force: true });
        } else {
          const response = await fetch(
            new URL(`/collections/${collection}`, endpoint),
            { method: "DELETE" },
          );
          assert.ok(response.ok || response.status === 404);
        }
      }
    },
  );
}
