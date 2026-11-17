#include "AethyrEditorChecks.h"

#if WITH_EDITOR
#include "Audio.h"
#include "MetasoundSource.h"
#include "Misc/FileHelper.h"
#include "Sound/SoundGenerator.h"
#endif

bool UAethyrEditorChecks::BakeMetaSound(USoundBase* Sound, const FString& Destination, float Seconds, bool bLoop)
{
#if WITH_EDITOR
	constexpr int32 Rate = 24000;
	constexpr int32 BlockSize = 512;
	constexpr int32 LoopFade = Rate / 50;
	UMetaSoundSource* Source = Cast<UMetaSoundSource>(Sound);
	if (!Source || !FMath::IsFinite(Seconds) || Seconds < 0.1f || Seconds > 60.f) { return false; }

	Source->InitResources();
	FSoundGeneratorInitParams Params{};
	Params.AudioDeviceID = INDEX_NONE;
	Params.AudioMixerNumOutputFrames = BlockSize;
	Params.NumChannels = 1;
	Params.NumFramesPerCallback = BlockSize;
	Params.SampleRate = Rate;
	Params.RenderMode = Audio::EDeviceRenderMode::Deterministic;
	const ISoundGeneratorPtr Generator = Source->CreateSoundGenerator(Params, {});
	if (!Generator || Generator->GetNumChannels() != 1) { return false; }

	const int32 Frames = FMath::RoundToInt(Seconds * Rate);
	const int32 Target = Frames + (bLoop ? LoopFade : 0);
	TArray<float> Samples;
	Samples.SetNumZeroed(Target);
	Generator->OnBeginGenerate();
	for (int32 At = 0; At < Target && !Generator->IsFinished(); At += BlockSize)
	{
		Generator->GetNextBuffer(Samples.GetData() + At, FMath::Min(BlockSize, Target - At), true);
	}
	Source->OnEndGenerate(Generator);
	// The extra samples continue naturally from the end of the loop. Blend
	// them into its head to avoid a discontinuity at the wrap point.
	if (bLoop)
	{
		for (int32 At = 0; At < LoopFade; ++At)
		{
			Samples[At] = FMath::Lerp(Samples[Frames + At], Samples[At], float(At) / float(LoopFade - 1));
		}
	}
	TArray<int16> PCM;
	PCM.SetNumUninitialized(Frames);
	bool bAudible = false;
	for (int32 At = 0; At < Frames; ++At)
	{
		if (!FMath::IsFinite(Samples[At])) { return false; }
		PCM[At] = int16(FMath::RoundToInt(FMath::Clamp(Samples[At], -1.f, 1.f) * 32767.f));
		bAudible |= PCM[At] != 0;
	}
	if (!bAudible) { return false; }
	TArray<uint8> Wave;
	SerializeWaveFile(Wave, reinterpret_cast<const uint8*>(PCM.GetData()), Frames * sizeof(int16), 1, Rate);
	return FFileHelper::SaveArrayToFile(Wave, *Destination);
#else
	return false;
#endif
}
