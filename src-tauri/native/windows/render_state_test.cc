#include "render_state.h"

#include <cassert>
#include <chrono>
#include <cmath>
#include <vector>

namespace {

bool Near(double actual, double expected) {
  return std::abs(actual - expected) < 0.000001;
}

RenderConfig Config(uint32_t fps, bool visible, double size = 600.0,
                    uint32_t pendingCount = 0, uint8_t visualStyle = 0) {
  RenderConfig config{};
  config.fps = fps;
  config.visible = visible;
  config.size = size;
  config.pendingCount = pendingCount;
  config.visualStyle = visualStyle;
  return config;
}

}  // namespace

int main() {
  {
    using Clock = std::chrono::steady_clock;
    const Clock::time_point hiddenAt(std::chrono::milliseconds(250));
    const auto firstHidden = HiddenRenderClock(hiddenAt);
    assert(firstHidden.lastFrame == hiddenAt);
    assert(firstHidden.nextFrame == Clock::time_point::max());
    const Clock::time_point hiddenAgain(std::chrono::milliseconds(5000));
    const auto repeatedHidden = HiddenRenderClock(hiddenAgain);
    assert(repeatedHidden.lastFrame == hiddenAgain);
    assert(repeatedHidden.nextFrame == Clock::time_point::max());
  }

  {
    RenderPresentationState presentation;
    RenderState state;
    state.apply(Config(0, false));
    presentation.requestVisible(true);
    int primeCalls = 0;
    int showCalls = 0;
    const bool failedPrerequisite = TryPrimeAndShow(
        presentation, false, [&primeCalls]() {
          ++primeCalls;
          return true;
        },
        [&showCalls]() {
          ++showCalls;
          return true;
        });
    state.setVisible(presentation.actuallyVisible());
    assert(!failedPrerequisite);
    assert(presentation.requestedVisible());
    assert(!presentation.actuallyVisible());
    assert(state.targetFps(60) == 0);
    assert(primeCalls == 0);
    assert(showCalls == 0);

    const bool failedPrime = TryPrimeAndShow(
        presentation, true, [&primeCalls]() {
          ++primeCalls;
          return false;
        },
        [&showCalls]() {
          ++showCalls;
          return true;
        });
    assert(!failedPrime);
    assert(!presentation.actuallyVisible());
    assert(primeCalls == 1);
    assert(showCalls == 0);

    const bool recovered = TryPrimeAndShow(
        presentation, true, [&primeCalls]() {
          ++primeCalls;
          return true;
        },
        [&showCalls]() {
          ++showCalls;
          return true;
        });
    state.setVisible(presentation.actuallyVisible());
    assert(recovered);
    assert(presentation.requestedVisible());
    assert(presentation.actuallyVisible());
    assert(state.targetFps(60) == 30);
    assert(primeCalls == 2);
    assert(showCalls == 1);

    int concealCalls = 0;
    int resizeCalls = 0;
    const bool resized = TryResizeWhileConcealed(
        presentation, [&concealCalls]() { ++concealCalls; },
        [&presentation, &resizeCalls]() {
          ++resizeCalls;
          assert(!presentation.actuallyVisible());
          return true;
        });
    assert(resized);
    assert(concealCalls == 1);
    assert(resizeCalls == 1);
    assert(presentation.requestedVisible());
    assert(!presentation.actuallyVisible());

    presentation.requestVisible(false);
    state.setVisible(presentation.actuallyVisible());
    assert(!presentation.requestedVisible());
    assert(state.targetFps(60) == 0);
    presentation.requestVisible(true);
    state.setVisible(presentation.actuallyVisible());
    assert(presentation.requestedVisible());
    assert(!presentation.actuallyVisible());
    assert(state.targetFps(60) == 0);
  }

  {
    SurfacePrimeState surface;
    surface.markPrimed();
    assert(surface.reveal());
    assert(surface.canRender());

    surface.conceal();
    assert(surface.primed());
    assert(surface.reveal());
    surface.conceal();
    surface.invalidatePrime();
    assert(!surface.primed());
    assert(!surface.reveal());
    assert(!surface.canRender());

    surface.markPrimed();
    assert(surface.reveal());
    assert(surface.canRender());
  }

  {
    constexpr int32_t busyResult =
        static_cast<int32_t>(static_cast<uint32_t>(0x887A000A));
    constexpr int32_t fatalResult =
        static_cast<int32_t>(static_cast<uint32_t>(0x80004005));
    assert(ClassifyPresentResult(0, busyResult) ==
           PresentDisposition::Presented);
    assert(ClassifyPresentResult(busyResult, busyResult) ==
           PresentDisposition::Retry);
    assert(ClassifyPresentResult(fatalResult, busyResult) ==
           PresentDisposition::DeviceFailure);

    SurfacePrimeState surface;
    surface.applyPrimePresent(PresentDisposition::Retry);
    assert(!surface.primed());
    assert(!surface.canRender());
    surface.applyPrimePresent(PresentDisposition::Presented);
    assert(surface.primed());
  }

  {
    RendererStatusState status;
    assert(status.value() == RendererAvailability::Unavailable);
    assert(!status.transition(RendererAvailability::Unavailable));
    assert(status.transition(RendererAvailability::Ready));
    assert(!status.transition(RendererAvailability::Ready));
    assert(status.transition(RendererAvailability::Unavailable));
  }

  {
    RendererRetryState retry;
    retry.request(100, true);
    assert(retry.due(100));
    retry.failed(100);
    retry.request(150, false);
    assert(!retry.due(199));
    assert(retry.due(200));
    retry.failed(200);
    assert(retry.due(400));
    retry.failed(400);
    assert(retry.due(800));
    retry.failed(800);
    assert(!retry.pending());
    assert(retry.attempts() == 4);
    retry.request(850, false);
    assert(!retry.pending());
    assert(retry.attempts() == 4);
    retry.request(900, true);
    assert(retry.attempts() == 0);
    assert(retry.due(900));
    retry.succeeded();
    assert(!retry.pending());
    assert(retry.attempts() == 0);
    retry.request(1000, false);
    retry.cancel();
    assert(!retry.pending());
  }

  {
    PresentationStatusState status;
    assert(!status.unavailable());
    assert(status.transitionUnavailable());
    assert(!status.transitionUnavailable());
    assert(status.unavailable());
    assert(status.transitionReady());
    assert(!status.transitionReady());
    assert(!status.unavailable());
  }

  {
    PresentationRetryState presentationRetry;
    RendererRetryState rendererRetry;
    std::vector<int> order;
    FinalizePresentationShow(
        presentationRetry, rendererRetry,
        [&order]() { order.push_back(1); },
        [&order]() { order.push_back(2); },
        [&order]() { order.push_back(3); },
        [&order]() { order.push_back(4); });
    assert((order == std::vector<int>{1, 2, 3, 4}));
  }

  {
    RendererSettingsInput input{};
    input.mode = 1;
    input.effectiveMode = 1;
    input.hasPosition = true;
    input.x = 100.0;
    input.y = 200.0;
    input.size = 220.0;
    input.displayId = 7;
    input.fps = 30;
    input.visible = true;
    input.reduceMotion = false;
    input.visualStyle = 1;

    RendererSettingsFingerprintState settings;
    assert(settings.shouldResetRetry(input));
    assert(!settings.shouldResetRetry(input));

    input.pendingCount = 3;
    assert(!settings.shouldResetRetry(input));
    input.requestPermission = true;
    assert(!settings.shouldResetRetry(input));

    input.effectiveMode = 0;
    assert(!settings.shouldResetRetry(input));
    input.x = 101.0;
    assert(!settings.shouldResetRetry(input));
    input.y = 201.0;
    assert(!settings.shouldResetRetry(input));
    input.displayId = 8;
    assert(!settings.shouldResetRetry(input));
    input.hasPosition = false;
    assert(!settings.shouldResetRetry(input));

    input.fps = 60;
    assert(settings.shouldResetRetry(input));
    input.visible = false;
    assert(settings.shouldResetRetry(input));
    input.mode = 0;
    assert(settings.shouldResetRetry(input));
    input.size = 300.0;
    assert(settings.shouldResetRetry(input));
    input.visualStyle = 0;
    assert(settings.shouldResetRetry(input));
    input.reduceMotion = true;
    assert(settings.shouldResetRetry(input));
  }

  {
    RenderPresentationState presentation;
    presentation.requestVisible(true);
    PresentationRetryState retry;
    int primeCalls = 0;
    int showCalls = 0;

    const bool first = TryPrimeAndShowWithRetry(
        presentation, retry, 0, true, false,
        [&primeCalls]() {
          ++primeCalls;
          return true;
        },
        [&showCalls]() {
          ++showCalls;
          return false;
        });
    assert(!first);
    assert(!presentation.actuallyVisible());
    assert(primeCalls == 1);
    assert(showCalls == 1);
    assert(!retry.due(99));
    assert(retry.due(100));

    const bool recovered = TryPrimeAndShowWithRetry(
        presentation, retry, 100, true, false,
        [&primeCalls]() {
          ++primeCalls;
          return true;
        },
        [&showCalls]() {
          ++showCalls;
          return true;
        });
    assert(recovered);
    assert(presentation.actuallyVisible());
    assert(primeCalls == 2);
    assert(showCalls == 2);
    assert(!retry.pending());
    assert(retry.attempts() == 0);

    retry.request(200, false);
    retry.failed(200);
    assert(!retry.due(201));
    assert(retry.request(201, true));
    assert(retry.due(201));
  }

  {
    RenderPresentationState presentation;
    presentation.requestVisible(true);
    PresentationRetryState retry;
    int showCalls = 0;
    for (uint64_t now : {0ULL, 100ULL, 300ULL, 700ULL}) {
      assert(!TryPrimeAndShowWithRetry(
          presentation, retry, now, true, false, []() { return true; },
          [&showCalls]() {
            ++showCalls;
            return false;
          }));
    }
    assert(showCalls == 4);
    assert(retry.exhausted());
    assert(ShouldNotifyPresentationUnavailable(retry));
    assert(!retry.pending());
    assert(retry.waitMilliseconds(700) ==
           std::numeric_limits<uint32_t>::max());
    assert(!TryPrimeAndShowWithRetry(
        presentation, retry, 1000, true, false, []() { return true; },
        [&showCalls]() {
          ++showCalls;
          return true;
        }));
    assert(showCalls == 4);
  }

  {
    using Clock = std::chrono::steady_clock;
    const Clock::time_point started(std::chrono::milliseconds(10000));
    const Clock::time_point deadline = NextRenderDeadline(started, 60);
    assert(deadline > started);
    assert(std::chrono::duration_cast<std::chrono::milliseconds>(
               deadline - started)
               .count() == 16);
    assert(NextRenderDeadline(started, 0) == Clock::time_point::max());
  }

  {
    using Clock = std::chrono::steady_clock;
    const Clock::time_point renderStarted(std::chrono::milliseconds(10000));
    const Clock::time_point renderCompleted =
        renderStarted + std::chrono::milliseconds(10);
    const Clock::time_point deadline = NextRenderDeadline(renderStarted, 60);
    assert(FrameWaitMilliseconds(deadline, renderCompleted) == 6);
    const Clock::time_point lateCompletion =
        renderStarted + std::chrono::milliseconds(20);
    assert(FrameWaitMilliseconds(deadline, lateCompletion) == 0);
  }

  {
    RenderState state;
    state.apply(Config(0, true));
    assert(state.targetFps(60) == 30);
    state.setHover(true);
    assert(state.targetFps(60) == 60);
    assert(Near(state.visualDiameter(), state.configuredDiameter()));
    state.setVisible(false);
    assert(state.targetFps(60) == 0);
  }

  {
    RenderState state;
    state.apply(Config(30, true));
    assert(state.targetFps(144) == 30);
    state.setVisualState(RenderVisualState::WaitingForAck);
    assert(state.targetFps(144) == 30);
    state.apply(Config(60, true));
    assert(state.targetFps(30) == 60);
  }

  {
    RenderState state;
    state.apply(Config(0, true, 119.0));
    assert(Near(state.configuredDiameter(), 120.0));
    state.apply(Config(0, true, 120.0));
    assert(Near(state.configuredDiameter(), 120.0));
    state.apply(Config(0, true, 220.0));
    assert(Near(state.configuredDiameter(), 220.0));
    state.apply(Config(0, true, 299.0));
    assert(Near(state.configuredDiameter(), 299.0));
    state.apply(Config(0, true, 300.0));
    assert(Near(state.configuredDiameter(), 300.0));
    state.apply(Config(0, true, 901.0));
    assert(Near(state.configuredDiameter(), 900.0));
    state.apply(Config(0, true, 540.0));
    assert(Near(state.configuredDiameter(), 540.0));
  }

  {
    RenderState state;
    state.apply(Config(0, true));
    state.advance(-1.0);
    assert(Near(state.frame().animationTime, 0.0));
    state.advance(0.25);
    assert(Near(state.frame().animationTime, 0.1));
    state.setHoverProgress(1.0);
    state.advance(0.25);
    assert(Near(state.frame().animationTime, 0.34));
    assert(state.frame().animationTime > 0.0);
  }

  {
    RenderState state;
    state.apply(Config(0, true));
    state.setHoverProgress(-1.0);
    assert(Near(state.frame().pullGain, 1.0));
    assert(Near(state.frame().rotationRate, 1.0));
    state.setHoverProgress(0.5);
    assert(Near(state.frame().pullGain, 1.35));
    assert(Near(state.frame().rotationRate, 1.7));
    state.setHoverProgress(2.0);
    assert(Near(state.frame().pullGain, 1.7));
    assert(Near(state.frame().rotationRate, 2.4));
    assert(Near(state.visualDiameter(), 600.0));
  }

  {
    RenderState state;
    state.apply(Config(0, true));
    state.setVisualState(RenderVisualState::WaitingForAck);
    state.advance(0.37);
    assert(Near(state.frame().ingestProgress, 0.5));
    assert(Near(state.frame().ejectProgress, 0.0));
    assert(Near(state.frame().successJetProgress, 0.0));

    state.setVisualState(RenderVisualState::SwallowAndSuccessJet);
    state.advance(0.37);
    assert(Near(state.frame().ingestProgress, 1.0));
    state.advance(0.25);
    assert(Near(state.frame().successJetProgress, 0.5));
    state.advance(0.25);
    assert(state.visualState() == RenderVisualState::Idle);
    assert(Near(state.frame().ingestProgress, 0.0));
    assert(Near(state.frame().successJetProgress, 0.0));
  }

  {
    RenderState state;
    state.apply(Config(0, true));
    state.setVisualState(RenderVisualState::SwallowAndEject);
    state.advance(0.74);
    assert(Near(state.frame().ingestProgress, 1.0));
    assert(Near(state.frame().ejectProgress, 0.0));
    state.advance(0.31);
    assert(Near(state.frame().ejectProgress, 0.5));
    state.advance(0.31);
    assert(state.visualState() == RenderVisualState::Idle);
  }

  {
    RenderState state;
    state.apply(Config(0, true));
    state.setVisualState(RenderVisualState::WaitingForAck);
    state.advance(2.0);
    assert(Near(state.frame().ingestProgress, 1.0));
    state.setVisualState(RenderVisualState::SwallowAndSuccessJet);
    assert(state.visualState() == RenderVisualState::SwallowAndSuccessJet);
    state.advance(0.25);
    assert(Near(state.frame().successJetProgress, 0.5));
  }

  {
    RenderState state;
    state.apply(Config(0, true, 600.0, 7, 0));
    assert(state.frame().pendingCount == 7);
    assert(state.frame().shaderStyle == 1);
    state.apply(Config(0, true, 600.0, 2, 1));
    assert(state.frame().pendingCount == 2);
    assert(state.frame().shaderStyle == 0);
  }

  return 0;
}
