import { PerspectiveCamera, Plane, Quaternion, Raycaster, Vector2, Vector3 } from "three";

const origin = new Vector3();

/** Rotate the entire camera frame around the map, without Euler-angle pole limits. */
export function orbitMap(camera: PerspectiveCamera, dx: number, dy: number) {
  const axis = new Vector3(dy, dx, 0);
  const angle = axis.length();
  if (!angle) return;
  axis.normalize().applyQuaternion(camera.quaternion);
  const rotation = new Quaternion().setFromAxisAngle(axis, -angle);
  camera.position.applyQuaternion(rotation);
  camera.quaternion.premultiply(rotation).normalize();
  camera.up.set(0, 1, 0).applyQuaternion(camera.quaternion);
  camera.updateMatrixWorld();
}

/** Dolly along the anchor ray; the anchor keeps the same screen position. */
export function zoomMap(
  camera: PerspectiveCamera,
  anchor: Vector3,
  factor: number,
  minDistance: number,
  maxDistance: number,
  centeredBounds = true,
) {
  if (!centeredBounds) {
    // Flat maps can be panned far from the origin; bound zoom relative to the focus.
    const distance = camera.position.distanceTo(anchor);
    if (!distance) return;
    factor = Math.max(minDistance / distance, Math.min(maxDistance / distance, factor));
    camera.position.sub(anchor).multiplyScalar(factor).add(anchor);
    camera.updateMatrixWorld();
    return;
  }
  const next = camera.position.clone().sub(anchor).multiplyScalar(factor).add(anchor);
  // Bound travel along the ray rather than clamping position off the cursor ray.
  const delta = next.clone().sub(camera.position);
  const distance = camera.position.length();
  const limit = factor < 1 ? minDistance : maxDistance;
  if ((factor < 1 && next.length() < limit) || (factor > 1 && next.length() > limit)) {
    const a = delta.lengthSq();
    const b = 2 * camera.position.dot(delta);
    const c = distance * distance - limit * limit;
    const discriminant = b * b - 4 * a * c;
    if (!a || discriminant < 0) return;
    const roots = [
      (-b - Math.sqrt(discriminant)) / (2 * a),
      (-b + Math.sqrt(discriminant)) / (2 * a),
    ];
    const t = roots.filter((value) => value >= 0 && value <= 1).sort((a, b) => a - b)[0];
    if (t === undefined) return;
    next.copy(camera.position).addScaledVector(delta, t);
  }
  camera.position.copy(next);
  camera.updateMatrixWorld();
}

/** Match rotation to the projected surface size, slowing down as the camera approaches it. */
export function orbitRadiansPerPixel(
  camera: PerspectiveCamera,
  radius: number,
  viewportHeight: number,
  flat = false,
) {
  const mapRadius = Math.max(1, radius);
  const surfaceDistance = Math.max(
    mapRadius * 0.01,
    camera.position.length() - (flat ? 0 : mapRadius),
  );
  const visibleHeight = 2 * Math.tan((camera.getEffectiveFOV() * Math.PI) / 360) * surfaceDistance;
  // The vertical FOV includes the aspect-ratio adjustment on narrow screens.
  return 2 * Math.min(Math.PI * 2, visibleHeight / mapRadius) / Math.max(1, viewportHeight);
}

/** Move the grabbed surface point with the pointer without changing the orbit center. */
export function panMap(camera: PerspectiveCamera, from: Vector2, to: Vector2, anchor: Vector3) {
  const start = cameraPlanePoint(camera, from, anchor);
  const end = cameraPlanePoint(camera, to, anchor);
  if (start && end) camera.position.add(start.sub(end));
  camera.updateMatrixWorld();
}

export function cameraPlanePoint(camera: PerspectiveCamera, ndc: Vector2, anchor = origin) {
  const raycaster = new Raycaster();
  raycaster.setFromCamera(ndc, camera);
  const plane = new Plane().setFromNormalAndCoplanarPoint(
    camera.getWorldDirection(new Vector3()),
    anchor,
  );
  return raycaster.ray.intersectPlane(plane, new Vector3());
}
