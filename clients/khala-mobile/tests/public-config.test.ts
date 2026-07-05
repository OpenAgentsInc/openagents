import { describe, expect, test } from "bun:test"

import {
  KhalaPublicConfigError,
  parseKhalaPublicConfig,
  type KhalaPublicConfigSource,
} from "../src/config/public-config"

const validSource = (): KhalaPublicConfigSource => ({
  android: {
    versionCode: 1,
  },
  extra: {
    khala: {
      apiBaseUrl: "https://openagents.com",
      syncBaseUrl: "https://openagents.com/",
      updatesOwner: "khala-mobile",
    },
  },
  ios: {
    buildNumber: "7",
  },
  name: "Khala Code",
  slug: "khala-mobile",
  updates: {
    url: "https://updates.openagents.com/khala-mobile/manifest",
  },
  version: "0.1.0",
})

describe("Khala public config", () => {
  test("parses only public Expo app metadata and endpoints", () => {
    expect(parseKhalaPublicConfig(validSource())).toEqual({
      androidVersionCode: 1,
      apiBaseUrl: "https://openagents.com",
      appName: "Khala Code",
      appSlug: "khala-mobile",
      appVersion: "0.1.0",
      iosBuildNumber: "7",
      syncBaseUrl: "https://openagents.com",
      updatesOwner: "khala-mobile",
      updatesUrl: "https://updates.openagents.com/khala-mobile/manifest",
    })
  })

  test("rejects missing or non-HTTPS public endpoints", () => {
    const source: KhalaPublicConfigSource = {
      ...validSource(),
      extra: {
      khala: {
        apiBaseUrl: "http://openagents.com",
        syncBaseUrl: "",
        updatesOwner: "khala-mobile",
      },
      },
    }

    expect(() => parseKhalaPublicConfig(source)).toThrow(KhalaPublicConfigError)

    try {
      parseKhalaPublicConfig(source)
    } catch (error) {
      expect(error).toBeInstanceOf(KhalaPublicConfigError)
      expect((error as KhalaPublicConfigError).issues).toContain(
        "extra.khala.apiBaseUrl must use https",
      )
      expect((error as KhalaPublicConfigError).issues).toContain(
        "extra.khala.syncBaseUrl must be a non-empty HTTPS URL",
      )
    }
  })

  test("rejects secret-shaped keys in bundled extra config", () => {
    const source: KhalaPublicConfigSource = {
      ...validSource(),
      extra: {
      khala: {
        apiBaseUrl: "https://openagents.com",
        nested: {
          bearerToken: "must-not-be-bundled",
        },
        syncBaseUrl: "https://openagents.com",
        updatesOwner: "khala-mobile",
      },
      },
    }

    expect(() => parseKhalaPublicConfig(source)).toThrow(
      /extra\.khala is public and must not contain secret-shaped keys: extra\.khala\.nested\.bearerToken/,
    )
  })
})
