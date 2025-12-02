export default {
    app: {
        name: "OpenAgents",
        identifier: "com.openagents.desktop",
        version: "0.1.0",
    },
    build: {
        views: {
            mainview: {
                entrypoint: "src/mainview/index.ts",
                external: [],
            },
        },
        copy: {
            "src/mainview/index.html": "views/mainview/index.html",
            "src/mainview/index.css": "views/mainview/index.css",
            "src/mainview/fonts": "views/mainview/fonts",
        },
        mac: {
            bundleCEF: false,
        },
        linux: {
            bundleCEF: false,
        },
        win: {
            bundleCEF: false,
        },
    },
};