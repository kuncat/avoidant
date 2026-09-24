import { describe, expect, it } from "vitest";
import { PerspectiveCamera, Vector2, Vector3 } from "three";
import { cameraPlanePoint, orbitMap, orbitRadiansPerPixel, panMap, zoomMap } from "./map-camera";

function cameraAt(position: Vector3) {
  const camera = new PerspectiveCamera(50, 1.5, 0.1, 1000);
  camera.position.copy(position);
  camera.lookAt(0, 0, 0);
  camera.updateMatrixWorld();
  return camera;
}

describe("map camera navigation", () => {
  it("rotates through both poles and keeps the map center fixed, even after cursor zoom", () => {
    const camera = cameraAt(new Vector3(0, 0, 10));
    zoomMap(camera, new Vector3(2, 1, 0), 0.8, 1, 100);
    const center = new Vector3().project(camera);
    const radius = camera.position.length();
    for (let step = 0; step < 100; step++) {
      const before = camera.position.clone();
      orbitMap(camera, 0, Math.PI / 25);
      expect(camera.position.distanceTo(before)).toBeGreaterThan(0.5);
      expect(camera.position.length()).toBeCloseTo(radius, 10);
      expect(new Vector3().project(camera).distanceTo(center)).toBeLessThan(1e-10);
      expect(camera.up.dot(camera.getWorldDirection(new Vector3()))).toBeCloseTo(0, 10);
    }
  });

  it("allows horizontal rotation from a directly overhead flat-map camera", () => {
    const camera = cameraAt(new Vector3(0, 10, 0));
    orbitMap(camera, 0.4, 0);
    expect(Math.abs(camera.position.x)).toBeGreaterThan(1);
    expect(camera.position.length()).toBeCloseTo(10);
  });

  it("keeps an off-center surface anchor under the cursor for zoom in and out", () => {
    const camera = cameraAt(new Vector3(3, 8, 10));
    const anchor = new Vector3(2, 1, 1);
    const screen = anchor.clone().project(camera);
    for (const factor of [0.7, 1.6, 0.9]) {
      zoomMap(camera, anchor, factor, 1, 100);
      const projected = anchor.clone().project(camera);
      expect(projected.x).toBeCloseTo(screen.x, 10);
      expect(projected.y).toBeCloseTo(screen.y, 10);
    }
  });

  it("stops at distance limits without slipping the cursor anchor", () => {
    const camera = cameraAt(new Vector3(0, 0, 10));
    const anchor = new Vector3(1, 0, 0);
    const screen = anchor.clone().project(camera);
    zoomMap(camera, anchor, 0.01, 3, 20);
    expect(camera.position.length()).toBeCloseTo(3);
    expect(anchor.clone().project(camera).x).toBeCloseTo(screen.x);
    zoomMap(camera, anchor, 100, 3, 20);
    expect(camera.position.length()).toBeCloseTo(20);
    expect(anchor.clone().project(camera).x).toBeCloseTo(screen.x);
  });

  it("bounds flat zoom around the focus after panning far from the map center", () => {
    const camera = cameraAt(new Vector3(0, 10, 0));
    camera.position.x = 200;
    camera.updateMatrixWorld();
    const anchor = new Vector3(200, 0, 0);
    zoomMap(camera, anchor, 0.001, 2, 30, false);
    expect(camera.position.y).toBeCloseTo(2);
    zoomMap(camera, anchor, 100, 2, 30, false);
    expect(camera.position.y).toBeCloseTo(30);
    expect(camera.position.x).toBe(200);
  });

  it("pans a zoomed surface point to the pointer and still orbits the map center", () => {
    const camera = cameraAt(new Vector3(0, 0, 10.5));
    const anchor = new Vector3(0, 0, 10);
    const destination = new Vector2(0.3, -0.2);
    const orientation = camera.quaternion.clone();
    panMap(camera, new Vector2(), destination, anchor);
    const projected = anchor.clone().project(camera);
    expect(projected.x).toBeCloseTo(destination.x);
    expect(projected.y).toBeCloseTo(destination.y);
    expect(camera.quaternion.equals(orientation)).toBe(true);
    const center = new Vector3().project(camera);
    orbitMap(camera, 0.2, 0.1);
    expect(new Vector3().project(camera).distanceTo(center)).toBeLessThan(1e-10);
  });

  it("slows close-up rotation while keeping surface movement consistent on narrow screens", () => {
    const movements: number[] = [];
    for (const [width, height] of [
      [1200, 800],
      [360, 800],
    ]) {
      const camera = cameraAt(new Vector3(0, 0, 25));
      camera.aspect = width / height;
      camera.fov = (2 * Math.atan(10 / (25 * Math.min(1, camera.aspect))) * 180) / Math.PI;
      camera.updateProjectionMatrix();
      const overviewSpeed = orbitRadiansPerPixel(camera, 10, height);
      camera.position.z = 10.5;
      camera.updateMatrixWorld();
      const closeSpeed = orbitRadiansPerPixel(camera, 10, height);
      expect(closeSpeed).toBeLessThan(overviewSpeed / 20);
      expect(closeSpeed).toBeGreaterThan(0);
      // The same drag should move the surface equally on wide and narrow screens,
      // regardless of the chosen overall rotation-speed multiplier.
      orbitMap(camera, closeSpeed * 2, 0);
      const screenX = (new Vector3(0, 0, 10).project(camera).x * width) / 2;
      movements.push(Math.abs(screenX));
    }
    expect(movements[0]).toBeGreaterThan(0);
    expect(movements[1]).toBeCloseTo(movements[0], 1);
  });

  it("finds a cursor anchor on the center plane when the cursor misses terrain", () => {
    const camera = cameraAt(new Vector3(0, 10, 0));
    const cursor = new Vector2(0.5, -0.3);
    const point = cameraPlanePoint(camera, cursor)!;
    const screen = point.clone().project(camera);
    expect(point.dot(camera.getWorldDirection(new Vector3()))).toBeCloseTo(0);
    expect(screen.x).toBeCloseTo(cursor.x);
    expect(screen.y).toBeCloseTo(cursor.y);
  });
});
