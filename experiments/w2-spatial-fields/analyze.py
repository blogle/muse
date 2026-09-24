#!/usr/bin/env python3
"""Experiment-local mesh metrics and simple equirectangular SVG screenshots."""
import json, math, pathlib, sys

root = pathlib.Path(__file__).parent
for filename in sys.argv[1:] or ["level2.json", "level5.json"]:
    state = json.loads((root / filename).read_text())
    mesh, fields = state["mesh"], state["fields"]
    positions, neighbors = mesh["positions"], mesh["neighbors"]
    land = fields["land"]["Bool"]
    elev, slope = fields["elevation"]["Scalar"], fields["slope"]["Scalar"]
    edges = [(i, j) for i, ns in enumerate(neighbors) for j in ns if i < j]
    seen, components = set(), []
    for start, is_land in enumerate(land):
        if start in seen: continue
        seen.add(start); stack = [start]; count = 0
        while stack:
            i = stack.pop(); count += 1
            for j in neighbors[i]:
                if j not in seen and land[j] == is_land:
                    seen.add(j); stack.append(j)
        components.append((is_land, count))
    nland = sum(land)
    land_components = sorted((n for kind, n in components if kind), reverse=True)
    ocean_components = sorted((n for kind, n in components if not kind), reverse=True)
    boundary = [sum(land[i] != land[j] for i, j in edges)]
    perimeter = boundary[0]
    corr_pairs = list(zip(slope, fields["plate_boundary"]["Scalar"]))
    x = [a for a, _ in corr_pairs]; y = [b for _, b in corr_pairs]
    mx, my = sum(x)/len(x), sum(y)/len(y)
    denom = math.sqrt(sum((a-mx)**2 for a in x)*sum((b-my)**2 for b in y))
    corr = sum((a-mx)*(b-my) for a,b in corr_pairs)/denom if denom else 0.0
    emean = sum(elev)/len(elev)
    var = sum((v-emean)**2 for v in elev)
    autocov = sum((elev[i]-emean)*(elev[j]-emean) for i,j in edges)/len(edges)
    autocorr = autocov/(var/len(elev)) if var else 0
    def angular_edge(i, j):
        a, b = positions[i], positions[j]
        na = math.sqrt(sum(v*v for v in a)); nb = math.sqrt(sum(v*v for v in b))
        return math.acos(max(-1, min(1, sum(a[k]*b[k] for k in range(3))/(na*nb))))
    lengths = sorted(angular_edge(i, j) for i, j in edges)
    h = lengths[len(lengths)//2]
    lag_hops = max(1, round(state["parameters"]["broad_radius"] / h))
    pairs=[]
    for start in range(len(land)):
        distances={start:0}; queue=[start]
        for current in queue:
            if distances[current] == lag_hops: continue
            for neighbor in neighbors[current]:
                if neighbor not in distances:
                    distances[neighbor]=distances[current]+1; queue.append(neighbor)
        pairs.extend((start,j) for j,d in distances.items() if d==lag_hops and start<j)
    lagcorr=sum((elev[i]-emean)*(elev[j]-emean) for i,j in pairs)/(len(pairs)*(var/len(elev))) if pairs and var else 0
    report = dict(file=filename, cells=len(land), land_fraction=nland/len(land), component_count=len(components),
                  largest_land_component_fraction=(land_components[0]/nland if nland else 0),
                  largest_ocean_component_fraction=(ocean_components[0]/(len(land)-nland) if nland < len(land) else 0),
                  land_components=len(land_components), ocean_components=len(ocean_components), perimeter_edge_count=perimeter,
                  boundary_slope_correlation=corr, elevation_neighbor_autocorrelation=autocorr,
                  median_edge_radians=h, broad_radius_graph_hops=lag_hops, broad_radius_lag_autocorrelation=lagcorr)
    print(json.dumps(report, sort_keys=True))
    # Vertex-color screenshot, projected to longitude/latitude (plate carrée).
    circles=[]
    for i,p in enumerate(positions):
        lon, lat = math.atan2(p[1],p[0]), math.asin(max(-1,min(1,p[2]/math.sqrt(sum(c*c for c in p)))))
        cx=(lon/math.pi+1)*500; cy=(0.5-lat/math.pi)*250
        if land[i]:
            t=max(0,min(1,(elev[i]+1.5)/3))
            color=f"#{int(70+100*t):02x}{int(95+85*t):02x}{int(45+35*t):02x}"
        else: color="#174b78"
        circles.append(f'<circle cx="{cx:.2f}" cy="{cy:.2f}" r="{max(1,650/math.sqrt(len(land))):.2f}" fill="{color}"/>')
    svg='<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="500" viewBox="0 0 1000 500"><rect width="1000" height="500" fill="#102b40"/>'+''.join(circles)+'</svg>'
    (root/(pathlib.Path(filename).stem+".svg")).write_text(svg)
