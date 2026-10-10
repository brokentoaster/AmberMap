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

        markersData.signs.forEach(sign => {
            const text = (sign.lines || [])
                .filter(Boolean)
                .join('\n') || 'Sign';
            const icon = L.divIcon({
                className: 'sign-marker-icon',
                html: '<div class="sign-marker">🪧</div>',
                iconSize: [18, 18],
                iconAnchor: [9, 9],
            });
            L.marker([sign.z, sign.x], { icon: icon })
                .bindTooltip(text.split('\n').map(l => l.replace(/</g, '&lt;')).join('<br>'), {
                    direction: 'top',
                })
                .addTo(markersLayer);
        });

        markersData.players.forEach(player => {
            L.circleMarker([player.z, player.x], {
                radius: 6,
                color: '#38bdf8',
                weight: 2,
                fillColor: '#0ea5e9',
                fillOpacity: 0.7,
            })
                .bindTooltip(
                    `${player.name || 'Player'}<br>` +
                    `X ${Math.round(player.x)} Y ${Math.round(player.y)} Z ${Math.round(player.z)}`, {
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
                    centerLng = data.center_x;
                    centerLat = data.center_z;
                } else {
                    centerLng = 0;
                    centerLat = 0;
                }
                if (data.min_block_x !== undefined && data.max_block_x !== undefined) {
                    const sw = L.latLng(data.min_block_z, data.min_block_x);
                    const ne = L.latLng(data.max_block_z, data.max_block_x);
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

        const scaleFactor = 1 / Math.pow(2, maxZoom);
        const AmberCRS = L.Util.extend({}, L.CRS.Simple, {
            transformation: new L.Transformation(scaleFactor, 0, scaleFactor, 0),
        });

        map = L.map('map', {
            crs: AmberCRS,
            minZoom: 0,
            maxZoom: maxZoom + 2,
            zoomControl: false,
            attributionControl: false,
        });

        if (worldBounds) {
            map.fitBounds(worldBounds, { maxZoom: Math.min(2, maxZoom) });
        } else {
            map.setView([centerLat, centerLng], Math.min(2, maxZoom));
        }

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
            const realBlockX = Math.round(latlng.lng);
            const realBlockZ = Math.round(latlng.lat);

            const chunkX = Math.floor(realBlockX / 16);
            const chunkZ = Math.floor(realBlockZ / 16);

            hudBlock.innerHTML = `X: ${realBlockX.toLocaleString()} &nbsp;|&nbsp; Z: ${realBlockZ.toLocaleString()}`;
            hudChunk.textContent = `${chunkX.toLocaleString()}, ${chunkZ.toLocaleString()}`;
            hudZoom.textContent = `Z: ${map.getZoom()}`;
        }

        updateCoords({ lat: centerLat, lng: centerLng });

        map.on('mousemove', function (e) {
            updateCoords(e.latlng);
        });

        map.on('zoomend', function () {
            hudZoom.textContent = `Z: ${map.getZoom()}`;
        });

        document.getElementById('btn-recenter').addEventListener('click', function () {
            if (worldBounds) {
                map.flyToBounds(worldBounds, { maxZoom: Math.min(2, maxZoom), duration: 0.6 });
            } else {
                map.flyTo([centerLat, centerLng], Math.min(2, maxZoom), { duration: 0.6 });
            }
        });

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
            loadDimension(this.dataset.dim);
        });
    });

    loadDimension(currentDimension);
})();
