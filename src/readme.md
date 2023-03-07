# Internals

## File view

This is mainly relevant for the `watch` mode.

```mermaid
graph TD;
  subgraph Watcher[watch]
    Watch[FS Notifier];
  end
  Watch-->|"*.rs & input.css"| TailW;
  Watch-->|"*.sass & *.scss"| Sass;
  Watch-->|"*.css"| Append;
  Watch-->|"*.rs"| WASM;
  Watch-->|"*.rs"| BIN;
  Watch-->|"assets/**"| Mirror;

  subgraph style
    TailW[Tailwind CSS];
    Sass;
    CSSProc[CSS Processor<br>Lightning CSS];
    Append{{append}};
  end

  TailW --> Append;
  Sass --> Append;
  Append --> CSSProc;

  subgraph rust
    WASM[Client WASM];
    BIN[Server BIN];
  end

  subgraph asset
    Mirror
  end

  subgraph update
    WOC[target/site/<br>Write-on-change FS];
    Live[Live Reload];
    Server;
  end

  Mirror -->|"site/**"| WOC;
  WASM -->|"site/pkg/app.wasm"| WOC;
  BIN -->|"server/app"| WOC;
  CSSProc -->|"site/pkg/app.css"| WOC;

  Live -.->|Port scan| Server;

  WOC -->|"target/server/app<br>site/**"| Server;
  WOC -->|"site/pkg/app.css, <br>client & server change"| Live;

  Live -->|"Reload all or<br>update app.css"| Browser

  Browser;
  Server -.- Browser;
```

## Concurrency view

Very approximate

```mermaid
stateDiagram-v2
    wasm: Build front
    bin: Build server
    style: Build style
    asset: Mirror assets
    serve: Run server

    state wait_for_start <<fork>>
      [*] --> wait_for_start
      wait_for_start --> wasm
      wait_for_start --> bin
      wait_for_start --> style
      wait_for_start --> asset

    reload: Reload
    state join_state <<join>>
      wasm --> join_state
      bin --> join_state
      style --> join_state
      asset --> join_state
    state if_state <<choice>>
        join_state --> if_state
        if_state --> reload: Ok
        if_state --> serve: Ok
        if_state --> [*] : Err
```
