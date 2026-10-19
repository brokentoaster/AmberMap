(function () {
    let currentDimension = 'overworld';
    let currentLayer = 'surface';
    let availableLayers = null;
    let tileFormat = 'webp';
    let maxZoom = 4;
    let centerLat = 0;
    let centerLng = 0;
    let worldBounds = null;
    let cacheBuster = Date.now();

    let map = null;
    let tileLayer = null;
    let markersLayer = null;
    let markersData = { signs: [], players: [] };
    let showPlayerMarkers = localStorage.getItem('showPlayerMarkers') !== 'false';
    let showSignMarkers = localStorage.getItem('showSignMarkers') !== 'false';
    let pendingView = null;
    let userInteracted = false;

    const NETHER_SCALE = 8;

    function coordScale() {
        return currentDimension === 'nether' ? NETHER_SCALE : 1;
    }

    // The nether pyramid is built in native nether blocks, but the viewer
    // presents the nether at overworld-equivalent scale (1 nether block =
    // 8 overworld blocks). Each extra display zoom level doubles on-screen
    // resolution, so 8x needs 3 extra display zoom levels on top of the
    // metadata max_zoom; tiles are fetched at maxNativeZoom = max_zoom and
    // upscaled, keeping every tile URL aligned with the rendered pyramid.
    function viewMaxZoom() {
        return maxZoom + (currentDimension === 'nether' ? 3 : 0);
    }

    function clampViewToBounds() {
        if (!worldBounds || !map) {
            return;
        }
        const c = map.getCenter();
        const lat = Math.min(Math.max(c.lat, worldBounds.getSouth()), worldBounds.getNorth());
        const lng = Math.min(Math.max(c.lng, worldBounds.getWest()), worldBounds.getEast());
        if (lat !== c.lat || lng !== c.lng) {
            map.setView([lat, lng], map.getZoom(), { animate: false });
        }
    }

    const blockIndexCache = new Map();
    let lastBlockIndexError = null;

    function sidecarSeg() {
        const layerSeg = currentLayer !== 'surface' ? currentLayer + '/' : '';
        return `${encodeURIComponent(currentDimension)}/${layerSeg}${maxZoom}`;
    }

    function clearBlockIndexCache() {
        blockIndexCache.clear();
    }

    function parseBlockIndex(data) {
        if (data.byteLength < 8) {
            return null;
        }
        const view = new DataView(data);
        let pos = 0;
        let magic = '';
        for (let i = 0; i < 4; i++) {
            magic += String.fromCharCode(view.getUint8(pos + i));
        }
        if (magic !== 'AMBI') {
            return null;
        }
        pos += 4;
        const version = view.getUint16(pos, true);
        pos += 2;
        if (version !== 1) {
            return null;
        }
        const nameCount = view.getUint16(pos, true);
        pos += 2;
        const bytes = new Uint8Array(data);
        const names = [];
        for (let n = 0; n < nameCount; n++) {
            let end = pos;
            while (end < bytes.length && bytes[end] !== 0) {
                end++;
            }
            if (end >= bytes.length) {
                return null;
            }
            names.push(new TextDecoder().decode(bytes.subarray(pos, end)));
            pos = end + 1;
        }
        const gridLen = 256 * 256;
        if (data.byteLength < pos + gridLen * 4) {
            return null;
        }
        const grid = new Uint16Array(gridLen);
        for (let i = 0; i < gridLen; i++) {
            grid[i] = view.getUint16(pos, true);
            pos += 2;
        }
        const topY = new Int16Array(gridLen);
        for (let i = 0; i < gridLen; i++) {
            topY[i] = view.getInt16(pos, true);
            pos += 2;
        }
        return { names, grid, topY };
    }

    const NO_BLOCK = 65535;

    function loadBlockIndex(baseTileX, baseTileZ) {
        // sidecar layout: <dim>[/<layer>]/<zoom>/<tileX>/<tileZ>.blocks.bin
        const key = `${sidecarSeg()}/${baseTileX}/${baseTileZ}`;
        if (blockIndexCache.has(key)) {
            return Promise.resolve(blockIndexCache.get(key));
        }
        const url = `/tiles/${key}.blocks.bin?t=${cacheBuster}`;
        return fetch(url)
            .then(res => {
                if (!res.ok) {
                    lastBlockIndexError = res.status === 404
                        ? null
                        : `HTTP ${res.status} for ${url}`;
                    return null;
                }
                lastBlockIndexError = null;
                return res.arrayBuffer();
            })
            .then(data => {
                let parsed = data ? parseBlockIndex(data) : null;
                if (!parsed && data && !lastBlockIndexError) {
                    lastBlockIndexError = 'sidecar decode failed';
                }
                if (parsed) {
                    blockIndexCache.set(key, parsed);
                } else {
                    // allow a retry later instead of caching a permanent miss
                    setTimeout(() => blockIndexCache.delete(key), 10000);
                }
                return parsed;
            })
            .catch(err => {
                lastBlockIndexError = `fetch failed: ${err}`;
                setTimeout(() => blockIndexCache.delete(key), 10000);
                return null;
            });
    }

    function updateBlockInfo(realBlockX, realBlockZ) {
        const el = document.getElementById('hud-block-info');
        if (!el) {
            return;
        }
        const s = coordScale();
        const nativeX = Math.round(realBlockX / s);
        const nativeZ = Math.round(realBlockZ / s);
        const tileX = Math.floor(nativeX / 256);
        const tileZ = Math.floor(nativeZ / 256);
        loadBlockIndex(tileX, tileZ).then(index => {
            if (!index) {
                el.textContent = 'no block';
                el.title = lastBlockIndexError || 'unrendered area';
                return;
            }
            const localX = ((nativeX % 256) + 256) % 256;
            const localZ = ((nativeZ % 256) + 256) % 256;
            const col = localZ * 256 + localX;
            const idx = index.grid[col];
            if (idx === NO_BLOCK || !index.names[idx]) {
                el.textContent = 'no block';
                el.title = 'empty column in ' + sidecarSeg() + '/' + tileX + '/' + tileZ;
                return;
            }
            const name = index.names[idx].replace(/^minecraft:/, '');
            const y = index.topY[col];
            const layerPart = currentLayer !== 'surface' ? ` \u00b7 ${currentLayer}` : '';
            el.textContent = `${name} (Y ${y})`;
            el.title = `X ${nativeX} Z ${nativeZ}${layerPart}`;
        });
    }

    function capturePendingView() {
        if (!map || !userInteracted) {
            pendingView = null;
            return;
        }
        const c = map.getCenter();
        pendingView = { lat: c.lat, lng: c.lng, zoom: map.getZoom() };
    }

    function loadMarkers(dim, layer) {
        markersData = { signs: [], players: [] };
        const layerSeg = layer && layer !== 'surface' ? encodeURIComponent(layer) + '/' : '';
        return fetch(`/tiles/${encodeURIComponent(dim)}/${layerSeg}markers.json?t=${cacheBuster}`)
            .then(res => (res.ok ? res.json() : null))
            .then(data => {
                if (data && Array.isArray(data.signs) && Array.isArray(data.players)) {
                    markersData = data;
                }
            })
            .catch(() => {
                /* markers are optional */
            });
    }

    function renderMarkers() {
        if (!map) {
            return;
        }
        if (markersLayer) {
            markersLayer.remove();
            markersLayer = null;
        }
        if (!markersData.signs.length && !markersData.players.length) {
            return;
        }

        markersLayer = L.layerGroup().addTo(map);
        const s = coordScale();

        markersData.signs.forEach(sign => {
            if (!showSignMarkers) {
                return;
            }
            const text = (sign.lines || [])
                .filter(Boolean)
                .join('\n') || 'Sign';
            const icon = L.divIcon({
                className: 'sign-marker-icon',
                html: '<div class="sign-marker">🪧</div>',
                iconSize: [18, 18],
                iconAnchor: [9, 9],
            });
            L.marker([sign.z * s, sign.x * s], { icon: icon })
                .bindTooltip(text.split('\n').map(l => l.replace(/</g, '&lt;')).join('<br>'), {
                    direction: 'top',
                })
                .addTo(markersLayer);
        });

        markersData.players.forEach(player => {
            if (!showPlayerMarkers) {
                return;
            }
            if (player.dimension && player.dimension !== currentDimension) {
                return;
            }
            const tooltip = `${player.name || 'Player'}<br>` +
                `X ${Math.round(player.x)} Y ${Math.round(player.y)} Z ${Math.round(player.z)}`;
            L.circleMarker([player.z, player.x], {
                radius: 6,
                color: '#38bdf8',
                weight: 2,
                fillColor: '#0ea5e9',
                fillOpacity: 0.7,
            })
                .bindTooltip(tooltip, {
                    direction: 'top',
                })
                .addTo(markersLayer);
        });
    }

    function loadDimension(dim, layer) {
        currentDimension = dim;
        const layerParam = layer ? '&layer=' + encodeURIComponent(layer) : '';
        return fetch('/api/status?dim=' + encodeURIComponent(dim) + layerParam)
            .then(res => res.json())
            .then(data => {
                if (data.format) {
                    tileFormat = data.format;
                }
                if (data.max_zoom !== undefined) {
                    maxZoom = data.max_zoom;
                }
                if (data.center_x !== undefined && data.center_z !== undefined) {
                    const s = coordScale();
                    centerLng = data.center_x * s;
                    centerLat = data.center_z * s;
                } else {
                    centerLng = 0;
                    centerLat = 0;
                }
                if (data.min_block_x !== undefined && data.max_block_x !== undefined) {
                    const s = coordScale();
                    const sw = L.latLng(data.min_block_z * s, data.min_block_x * s);
                    const ne = L.latLng(data.max_block_z * s, data.max_block_x * s);
                    worldBounds = L.latLngBounds(sw, ne);
                } else {
                    worldBounds = null;
                }
            })
            .catch(() => {
                tileFormat = 'webp';
                maxZoom = 4;
                centerLat = 0;
                centerLng = 0;
                worldBounds = null;
            })
            .then(() => {
                if (!layer) {
                    return fetch('/api/status?dim=' + encodeURIComponent(dim))
                        .then(res => res.json())
                        .then(status => {
                            availableLayers = Array.isArray(status.layers) && status.layers.length > 0
                                ? status.layers
                                : null;
                            if (availableLayers && !availableLayers.some(l => l.id === currentLayer)) {
                                currentLayer = 'surface';
                            }
                        })
                        .catch(() => {
                            availableLayers = null;
                        });
                }
            })
            .then(() => {
                initMap();
            })
            .then(() => loadMarkers(currentDimension, currentLayer))
            .then(renderMarkers);
    }

    function initMap() {
        if (map) {
            map.remove();
        }

        const scaleFactor = 1 / Math.pow(2, viewMaxZoom());
        const AmberCRS = L.Util.extend({}, L.CRS.Simple, {
            transformation: new L.Transformation(scaleFactor, 0, scaleFactor, 0),
        });

        map = L.map('map', {
            crs: AmberCRS,
            minZoom: 0,
            maxZoom: viewMaxZoom() + 2,
            zoomControl: false,
            attributionControl: false,
        });

        if (pendingView) {
            const targetZoom = Math.min(pendingView.zoom, viewMaxZoom() + 2);
            map.setView([pendingView.lat, pendingView.lng], targetZoom, { animate: false });
            clampViewToBounds();
            pendingView = null;
        } else if (worldBounds) {
            map.fitBounds(worldBounds, { maxZoom: Math.min(2, maxZoom) });
        } else {
            map.setView([centerLat, centerLng], Math.min(2, maxZoom));
        }

        map.on('dragstart', function () {
            userInteracted = true;
        });
        map.on('zoomstart', function () {
            userInteracted = true;
        });

        L.control.zoom({ position: 'topright' }).addTo(map);

        const AmberTileLayer = L.TileLayer.extend({
            getTileUrl: function (coords) {
                const z = Math.min(coords.z, maxZoom);
                const x = coords.x;
                const y = coords.y;
                const layerSeg = currentLayer === 'surface' ? '' : currentLayer + '/';
                return `/tiles/${currentDimension}/${layerSeg}${z}/${x}/${y}.${tileFormat}?t=${cacheBuster}`;
            }
        });

        tileLayer = new AmberTileLayer('', {
            tileSize: 256,
            noWrap: true,
            maxNativeZoom: maxZoom,
            errorTileUrl: 'data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256"><rect width="256" height="256" fill="rgba(15,23,42,0.6)"/></svg>',
        }).addTo(map);

        const hudBlock = document.getElementById('hud-block-coords');
        const hudChunk = document.getElementById('hud-chunk-coords');
        const hudZoom = document.getElementById('hud-zoom-level');

        function updateCoords(latlng) {
            const s = coordScale();
            const realBlockX = Math.round(latlng.lng);
            const realBlockZ = Math.round(latlng.lat);

            const chunkX = Math.floor(realBlockX / 16);
            const chunkZ = Math.floor(realBlockZ / 16);

            if (s === 1) {
                hudBlock.innerHTML = `X: ${realBlockX.toLocaleString()} &nbsp;|&nbsp; Z: ${realBlockZ.toLocaleString()}`;
                hudChunk.textContent = `${chunkX.toLocaleString()}, ${chunkZ.toLocaleString()}`;
            } else {
                const netherX = Math.round(latlng.lng / s);
                const netherZ = Math.round(latlng.lat / s);
                hudBlock.innerHTML =
                    `X: ${realBlockX.toLocaleString()} &nbsp;|&nbsp; Z: ${realBlockZ.toLocaleString()}` +
                    ` &nbsp;(Nether: ${netherX.toLocaleString()}, ${netherZ.toLocaleString()})`;
                hudChunk.textContent =
                    `${chunkX.toLocaleString()}, ${chunkZ.toLocaleString()}` +
                    ` (Nether: ${Math.floor(netherX / 16).toLocaleString()}, ` +
                    `${Math.floor(netherZ / 16).toLocaleString()})`;
            }
            hudZoom.textContent = `Z: ${map.getZoom()}`;
            updateBlockInfo(realBlockX, realBlockZ);
        }

        updateCoords({ lat: centerLat, lng: centerLng });

        map.on('mousemove', function (e) {
            updateCoords(e.latlng);
        });

        map.on('zoomend', function () {
            hudZoom.textContent = `Z: ${map.getZoom()}`;
        });

        document.getElementById('btn-recenter').addEventListener('click', function () {
            userInteracted = true;
            if (worldBounds) {
                map.flyToBounds(worldBounds, { maxZoom: Math.min(2, maxZoom), duration: 0.6 });
            } else {
                map.flyTo([centerLat, centerLng], Math.min(2, maxZoom), { duration: 0.6 });
            }
        });

        const playersBtn = document.getElementById('btn-toggle-players');
        playersBtn.classList.toggle('active', showPlayerMarkers);
        playersBtn.addEventListener('click', function () {
            showPlayerMarkers = !showPlayerMarkers;
            localStorage.setItem('showPlayerMarkers', String(showPlayerMarkers));
            playersBtn.classList.toggle('active', showPlayerMarkers);
            renderMarkers();
        });

        const signsBtn = document.getElementById('btn-toggle-signs');
        if (signsBtn) {
            signsBtn.classList.toggle('active', showSignMarkers);
            signsBtn.addEventListener('click', function () {
                showSignMarkers = !showSignMarkers;
                localStorage.setItem('showSignMarkers', String(showSignMarkers));
                signsBtn.classList.toggle('active', showSignMarkers);
                renderMarkers();
            });
        }

        document.querySelectorAll('.dim-btn').forEach(btn => {
            btn.classList.toggle('active', btn.dataset.dim === currentDimension);
        });

        updateLayerSelector();
    }

    function updateLayerSelector() {
        const sel = document.getElementById('layer-selector');
        if (!sel) {
            return;
        }
        if (!availableLayers || availableLayers.length < 2) {
            sel.classList.add('hidden');
            return;
        }
        sel.classList.remove('hidden');
        sel.innerHTML = '';
        availableLayers.forEach(l => {
            const btn = document.createElement('button');
            btn.className = 'dim-btn layer-btn';
            btn.dataset.layer = l.id;
            btn.textContent = l.label || l.id;
            btn.classList.toggle('active', l.id === currentLayer);
            btn.addEventListener('click', function () {
                if (this.dataset.layer === currentLayer) {
                    return;
                }
                currentLayer = this.dataset.layer;
                cacheBuster = Date.now();
                capturePendingView();
                clearBlockIndexCache();
                loadDimension(currentDimension, currentLayer);
            });
            sel.appendChild(btn);
        });
    }

    document.querySelectorAll('.dim-btn').forEach(btn => {
        btn.addEventListener('click', function () {
            if (this.dataset.dim === currentDimension) {
                return;
            }
            document.querySelectorAll('.dim-btn').forEach(b => b.classList.remove('active'));
            this.classList.add('active');
            currentLayer = 'surface';
            cacheBuster = Date.now();
            capturePendingView();
            clearBlockIndexCache();
            loadDimension(this.dataset.dim);
        });
    });

    loadDimension(currentDimension);
})();
