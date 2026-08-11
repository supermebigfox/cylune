#include "animation.h"
#include "render_state.h"

#include <cassert>
#include <cmath>

namespace {

bool Near(double actual, double expected) {
  return std::abs(actual - expected) < 0.000001;
}

}  // namespace

int main() {
  assert(SwallowProgress(0.74) == 1.0);
  assert(EjectProgress(0.74) == 0.0);
  assert(EjectProgress(1.36) == 1.0);
  assert(SuccessJetProgress(1.24) == 1.0);

  assert(Near(SmoothstepEase(0.5), 0.5));
  assert(OrbitScale(0.0) == 1.0);
  assert(OrbitScale(1.0) == 0.0);

  const HoverUniforms hover = HoverEffect(1.0);
  assert(hover.rotationRate == 2.4f);
  assert(hover.pullGain == 1.7f);
  assert(hover.visualDiameterScale == 1.0f);
  assert(HoverVisualDiameter(600.0, hover) == 600.0);

  const AnimationUniforms ingest =
      ResolveAnimation(AnimationState::Swallow, 0.37);
  assert(Near(ingest.ingestProgress, 0.5));
  assert(Near(ingest.ejectProgress, 0.0));
  assert(Near(ingest.successJetProgress, 0.0));
  assert(Near(ingest.orbitScale, std::pow(0.5, 1.18)));

  const AnimationUniforms eject =
      ResolveAnimation(AnimationState::Eject, 1.05);
  assert(Near(eject.ingestProgress, 1.0));
  assert(Near(eject.ejectProgress, 0.5));
  assert(Near(eject.successJetProgress, 0.0));

  const AnimationUniforms jet =
      ResolveAnimation(AnimationState::SuccessJet, 0.99);
  assert(Near(jet.ingestProgress, 1.0));
  assert(Near(jet.ejectProgress, 0.0));
  assert(Near(jet.successJetProgress, 0.5));

  RenderState thirtyFps;
  thirtyFps.apply({30, true, 600.0, 0, 0});
  thirtyFps.setVisualState(RenderVisualState::WaitingForAck);
  for (int frame = 0; frame < 9; ++frame) thirtyFps.advance(1.0 / 30.0);
  thirtyFps.advance(0.07);

  RenderState sixtyFps;
  sixtyFps.apply({60, true, 600.0, 0, 0});
  sixtyFps.setVisualState(RenderVisualState::WaitingForAck);
  for (int frame = 0; frame < 18; ++frame) sixtyFps.advance(1.0 / 60.0);
  sixtyFps.advance(0.07);

  const AnimationUniforms expected =
      ResolveAnimation(AnimationState::Swallow, 0.37);
  assert(Near(thirtyFps.frame().ingestProgress, expected.ingestProgress));
  assert(Near(sixtyFps.frame().ingestProgress, expected.ingestProgress));

  RenderState clampedTime;
  clampedTime.apply({0, true, 600.0, 0, 0});
  clampedTime.advance(-1.0);
  assert(Near(clampedTime.frame().animationTime, 0.0));
  clampedTime.advance(1.0);
  assert(Near(clampedTime.frame().animationTime, 0.1));

  clampedTime.setHoverProgress(-1.0);
  assert(Near(clampedTime.frame().rotationRate, 1.0));
  clampedTime.setHoverProgress(2.0);
  assert(Near(clampedTime.frame().rotationRate, 2.4));
  assert(Near(clampedTime.frame().pullGain, 1.7));
  assert(Near(clampedTime.visualDiameter(), 600.0));

  RenderState acknowledgedSuccess;
  acknowledgedSuccess.apply({0, true, 600.0, 0, 0});
  acknowledgedSuccess.setVisualState(RenderVisualState::WaitingForAck);
  acknowledgedSuccess.advance(2.0);
  acknowledgedSuccess.setVisualState(
      RenderVisualState::SwallowAndSuccessJet);
  assert(Near(acknowledgedSuccess.frame().ingestProgress, 1.0));
  assert(Near(acknowledgedSuccess.frame().successJetProgress, 0.0));
  acknowledgedSuccess.advance(0.5);
  assert(acknowledgedSuccess.visualState() == RenderVisualState::Idle);

  RenderState acknowledgedEject;
  acknowledgedEject.apply({0, true, 600.0, 0, 0});
  acknowledgedEject.setVisualState(RenderVisualState::WaitingForAck);
  acknowledgedEject.advance(2.0);
  acknowledgedEject.setVisualState(RenderVisualState::SwallowAndEject);
  assert(Near(acknowledgedEject.frame().ingestProgress, 1.0));
  assert(Near(acknowledgedEject.frame().ejectProgress, 0.0));
  acknowledgedEject.advance(0.62);
  assert(acknowledgedEject.visualState() == RenderVisualState::Idle);

  return 0;
}
